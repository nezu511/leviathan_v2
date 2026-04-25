#![allow(dead_code)]
pub mod evm;
pub mod leviathan;
pub mod my_trait;
pub mod test;

use alloy_primitives::{Address, Bytes, U256, hex, keccak256, uint};
use alloy_rlp::{Encodable, Header};
use bytes::BytesMut;
use rand::rngs::OsRng;
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1v15::Pkcs1v15Sign, traits::PublicKeyParts};
use secp256k1::{Message, Secp256k1, SecretKey};
use sha2::Sha256;

use leviathan::leviathan::LEVIATHAN;
use leviathan::structs::{BlockHeader, Transaction, VersionId};
use leviathan::world_state::{Account, WorldState};
use my_trait::leviathan_trait::{State, TransactionExecution};
use leviathan_v2::solidity_utils::sign_tx_properly;


fn main() {
    // ログレベルを指定して詳細な動きを追えるようにします
    let _ = tracing_subscriber::fmt::init();
    let version = VersionId::Petersburg;

    // 前回の完璧なアーキテクチャ修正が活きる WorldState の初期化！
    let mut state = WorldState::new();
    let mut leviathan = LEVIATHAN::new(version);
    let secp = Secp256k1::new();

    // ---------------------------------------------------------
    // 1. 送信者 (EOA) の準備
    // ---------------------------------------------------------
    let secret_key = SecretKey::from_slice(&hex!(
        "45cd63531c3c97355b9275e7a9e6323c2a937a07011d8825e36873c907b29a28"
    ))
    .unwrap();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let serialized_pub = public_key.serialize_uncompressed();
    let pub_hash = keccak256(&serialized_pub[1..65]);
    let sender_addr = Address::from_slice(&pub_hash[12..32]);

    let mut sender_acc = Account::new();
    sender_acc.balance = uint!(100_000_000_000_000_000_000_U256); // 100 ETH

    // 自作した完璧な init_mpt_account メソッドで安全に状態を構築
    state.init_mpt_account(&sender_addr, &sender_acc);

    // ---------------------------------------------------------
    // 2. RSAデータの準備 (マイナンバーシミュレーション)
    // ---------------------------------------------------------
    let priv_key = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
    let pub_key = RsaPublicKey::from(&priv_key);
    let msg_hash = keccak256("Leviathan Vote");
    let signature = priv_key
        .sign(Pkcs1v15Sign::new::<Sha256>(), msg_hash.as_slice())
        .unwrap();

    // ---------------------------------------------------------
    // 3. ペイロードデータの構築（Solidityの忖度なし！生データ直結！）
    // ---------------------------------------------------------
    // 関数セレクタもオフセットパディングも不要。純粋なデータを連結するだけ。
    let payload_data = [
        signature.as_slice(),       // 256 bytes
        &pub_key.n().to_bytes_be(), // 256 bytes
        msg_hash.as_slice(),        // 32 bytes
        &pub_key.e().to_bytes_be(), // 可変長
    ]
    .concat();

    // ---------------------------------------------------------
    // 4. トランザクションの構築と正当な署名
    // ---------------------------------------------------------
    // 宛先は直接プレコンパイル (0x000000000000000000000000000000000000000a) を指定
    let precompile_addr = Address::with_last_byte(0x0a);

    let t_nonce = 0u64;
    let t_price = uint!(1_U256);
    let t_gas_limit = uint!(1_000_000_U256); // RSA処理に十分なガスを用意
    let t_value = U256::ZERO;

    let (v, r, s) = sign_tx_properly(
        U256::from(t_nonce),
        t_price,
        t_gas_limit,
        Some(precompile_addr), // ここで 0x0a を指定！
        t_value,
        &payload_data, // 生データをそのまま送信
        &secret_key,
    );

    let transaction = Transaction {
        data: payload_data,
        t_to: Some(precompile_addr),
        t_gas_limit,
        t_price,
        t_value,
        t_nonce: t_nonce as usize,
        t_w: v,
        t_r: r,
        t_s: s,
    };

    // ---------------------------------------------------------
    // 5. 実行
    // ---------------------------------------------------------
    let block = BlockHeader {
        h_beneficiary: Address::repeat_byte(0xfe),
        h_timestamp: uint!(1600000000_U256),
        h_number: uint!(1_U256),
        h_prevrandao: U256::ZERO,
        h_gaslimit: uint!(30_000_000_U256),
        h_basefee: U256::ZERO,
    };

    println!(" Starting Direct EVM Execution to RSA Precompile (0x0a)...");
    match leviathan.execution(&mut state, transaction, &block) {
        Ok((gas, _)) => println!(
            " Success! Precompile verified the signature. Remaining Gas: {}",
            gas
        ),
        Err((gas, _)) => println!(" Failed. Precompile call reverted. Gas consumed: {}", gas),
    }
}
