use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::{BlockHeader, Log, Transaction, VersionId};
use crate::leviathan::world_state::{Account, WorldState};
use crate::my_trait::leviathan_trait::{State, TransactionExecution};

use alloy_primitives::{Address, Bytes, U256, hex, keccak256, uint};
use alloy_rlp::{Encodable, Header};
use bytes::BytesMut;
use secp256k1::{Message, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use std::fs;

/// イエローペーパー Appendix F に基づくトランザクション署名関数
pub fn sign_tx_properly(
    nonce: U256,
    gas_price: U256,
    gas_limit: U256,
    to: Option<Address>,
    value: U256,
    data: &[u8],
    secret_key: &SecretKey,
) -> (U256, U256, U256) {
    let mut payload_length = 0;
    payload_length += nonce.length();
    payload_length += gas_price.length();
    payload_length += gas_limit.length();
    let to_slice = match &to {
        Some(addr) => addr.0.as_slice(),
        None => &[],
    };
    payload_length += to_slice.length();
    payload_length += value.length();
    payload_length += data.length();

    let mut out = BytesMut::with_capacity(payload_length + 10);
    Header {
        list: true,
        payload_length,
    }
    .encode(&mut out);

    nonce.encode(&mut out);
    gas_price.encode(&mut out);
    gas_limit.encode(&mut out);
    to_slice.encode(&mut out);
    value.encode(&mut out);
    data.encode(&mut out);

    let rlp_encoded = out.freeze();
    let hash = keccak256(&rlp_encoded);

    let secp = Secp256k1::new();
    let message = Message::from_digest_slice(&hash.0).unwrap();
    let sig = secp.sign_ecdsa_recoverable(message, secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let r = U256::from_be_slice(&sig_bytes[0..32]);
    let s = U256::from_be_slice(&sig_bytes[32..64]);
    let v = U256::from(i32::from(recovery_id) as u64 + 27);

    (v, r, s)
}

pub fn init_leviathan() {}

pub fn deploy_contract(
    leviathan: &mut LEVIATHAN,
    state: &mut WorldState,
    sender_secretkey: &SecretKey,
    file_path: &str,
    eth: U256,
    gas_price: U256,
    gas_limit: U256,
) -> Result<Address, ()> {
    //1. ファイルからバイトコードを取得
    let Ok(hex_data) = fs::read_to_string(file_path) else {
        tracing::warn!("[deploy_contract] ファイル読み込みエラー");
        return Err(());
    };

    //2. プレフィックスの除去とデコード
    let Ok(init_code) = hex::decode(hex_data.trim().trim_start_matches("0x")) else {
        tracing::warn!("[deploy_contract] コードのデコードに失敗");
        return Err(());
    };

    //3. secretkeyからアドレスを作成
    let secp = Secp256k1::new();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &sender_secretkey);
    let serialized_pub = public_key.serialize_uncompressed();
    let pub_hash = keccak256(&serialized_pub[1..65]);
    let sender_addr = Address::from_slice(&pub_hash[12..32]);

    //4. transactionを構築
    let sender_nonce = state.get_nonce(&sender_addr).unwrap_or(0);

    let (v, r, s) = sign_tx_properly(
        U256::from(sender_nonce),
        gas_price,
        gas_limit,
        None,
        eth,
        &init_code,
        &sender_secretkey,
    );

    let transaction = Transaction {
        data: init_code,
        t_to: None,
        t_gas_limit: gas_limit,
        t_price: gas_price,
        t_value: eth,
        t_nonce: sender_nonce as usize,
        t_w: v,
        t_r: r,
        t_s: s,
    };

    //5. ブロックヘッダー構築
    let block = BlockHeader {
        h_beneficiary: Address::repeat_byte(0xfe),
        h_timestamp: uint!(1600000000_U256),
        h_number: uint!(1_U256),
        h_prevrandao: U256::ZERO,
        h_gaslimit: uint!(30_000_000_U256),
        h_basefee: U256::ZERO,
    };

    //実行
    let Ok((gas, log_list)) = leviathan.execution(state, transaction, &block) else {
        println!(" Contract Creation Failed. ");
        return Err(());
    };
    println!(
        " Success! Precompile verified the signature. Remaining Gas: {}",
        gas
    );

    // 構築されたコントラクトのアドレスを計算
    // 1. 各要素のRLPペイロード長を事前計算
    let mut payload_length = 0;
    payload_length += sender_addr.0.as_slice().length();
    payload_length += sender_nonce.length();
    // 2. 必要なメモリを一括で確保し、リストのヘッダーを書き込む
    let mut out = BytesMut::with_capacity(payload_length + 10);
    Header {
        list: true,
        payload_length,
    }
    .encode(&mut out);
    // 3. データを順次エンコード
    sender_addr.0.as_slice().encode(&mut out);
    sender_nonce.encode(&mut out);
    // 4. ハッシュ化の前準備として Vec<u8> に変換して返す
    let rlp_byte = out.to_vec();
    let mut hasher = Keccak256::new();
    hasher.update(&rlp_byte);
    let result: [u8; 32] = hasher.finalize().into();
    let mut tmp = [0u8; 20];
    tmp.copy_from_slice(&result[12..32]);
    let contract_address = Address::new(tmp);

    return Ok(contract_address);
}

// src/solidity_utils.rs

pub fn call_contract(
    leviathan: &mut LEVIATHAN,
    state: &mut WorldState,
    sender_secretkey: &SecretKey,
    contract_addr: Address,
    data: Vec<u8>,
    eth: U256,
    gas_price: U256,
    gas_limit: U256,
) -> Result<Vec<Log>, ()> {
    // 1. 送信者アドレスとNonce取得
    let secp = Secp256k1::new();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, sender_secretkey);
    let pub_hash = keccak256(&public_key.serialize_uncompressed()[1..65]);
    let sender_addr = Address::from_slice(&pub_hash[12..32]);
    let sender_nonce = state.get_nonce(&sender_addr).unwrap_or(0);

    // 2. 署名とトランザクション構築
    let (v, r, s) = sign_tx_properly(
        U256::from(sender_nonce),
        gas_price,
        gas_limit,
        Some(contract_addr),
        eth,
        &data,
        sender_secretkey,
    );

    let transaction = Transaction {
        data,
        t_to: Some(contract_addr),
        t_gas_limit: gas_limit,
        t_price: gas_price,
        t_value: eth,
        t_nonce: sender_nonce as usize,
        t_w: v,
        t_r: r,
        t_s: s,
    };

    // 3. ブロックヘッダー
    let block = BlockHeader {
        h_beneficiary: Address::repeat_byte(0xfe),
        h_timestamp: uint!(1600000000_U256),
        h_number: uint!(1_U256),
        h_prevrandao: U256::ZERO,
        h_gaslimit: uint!(30_000_000_U256),
        h_basefee: U256::ZERO,
    };

    // 4. 実行
    match leviathan.execution(state, transaction, &block) {
        Ok((remaining_gas, output)) => {
            println!(" Call Success! Remaining Gas: {}", remaining_gas);
            Ok(output) // コントラクトからの戻り値を返す
        }
        Err(_) => {
            println!(" Call Failed.");
            Err(())
        }
    }
}

pub fn deploy_contract_raw(
    leviathan: &mut LEVIATHAN,
    state: &mut WorldState,
    sender_secretkey: &SecretKey,
    init_code: Vec<u8>, // 🌟 ここに「バイトコード + 引数」を渡す
    eth: U256,
    gas_price: U256,
    gas_limit: U256,
) -> Result<Address, ()> {
    // 1. 送信者アドレスとノンスの取得
    let secp = Secp256k1::new();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &sender_secretkey);
    let pub_hash = keccak256(&public_key.serialize_uncompressed()[1..65]);
    let sender_addr = Address::from_slice(&pub_hash[12..32]);
    let sender_nonce = state.get_nonce(&sender_addr).unwrap_or(0);

    // 2. 署名とトランザクション構築
    let (v, r, s) = sign_tx_properly(
        U256::from(sender_nonce),
        gas_price,
        gas_limit,
        None, // デプロイなので to は None
        eth,
        &init_code,
        &sender_secretkey,
    );

    let transaction = Transaction {
        data: init_code,
        t_to: None,
        t_gas_limit: gas_limit,
        t_price: gas_price,
        t_value: eth,
        t_nonce: sender_nonce as usize,
        t_w: v,
        t_r: r,
        t_s: s,
    };

    // 3. ブロックヘッダーの準備
    let block = BlockHeader {
        h_beneficiary: Address::repeat_byte(0xfe),
        h_timestamp: uint!(1600000000_U256),
        h_number: uint!(1_U256),
        h_prevrandao: U256::ZERO,
        h_gaslimit: uint!(30_000_000_U256),
        h_basefee: U256::ZERO,
    };

    // 4. Leviathan で実行
    //実行
    let Ok((gas, log_list)) = leviathan.execution(state, transaction, &block) else {
        println!(" Contract Creation Failed. ");
        return Err(());
    };
    println!(
        " Success! Precompile verified the signature. Remaining Gas: {}",
        gas
    );

    // 構築されたコントラクトのアドレスを計算
    // 1. 各要素のRLPペイロード長を事前計算
    let mut payload_length = 0;
    payload_length += sender_addr.0.as_slice().length();
    payload_length += sender_nonce.length();
    // 2. 必要なメモリを一括で確保し、リストのヘッダーを書き込む
    let mut out = BytesMut::with_capacity(payload_length + 10);
    Header {
        list: true,
        payload_length,
    }
    .encode(&mut out);
    // 3. データを順次エンコード
    sender_addr.0.as_slice().encode(&mut out);
    sender_nonce.encode(&mut out);
    // 4. ハッシュ化の前準備として Vec<u8> に変換して返す
    let rlp_byte = out.to_vec();
    let mut hasher = Keccak256::new();
    hasher.update(&rlp_byte);
    let result: [u8; 32] = hasher.finalize().into();
    let mut tmp = [0u8; 20];
    tmp.copy_from_slice(&result[12..32]);
    let contract_address = Address::new(tmp);

    return Ok(contract_address);
}
