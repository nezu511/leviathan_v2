#![allow(dead_code)]
pub mod evm;
pub mod leviathan;
pub mod my_trait;
pub mod test;

use alloy_primitives::{Address, U256, hex, keccak256, uint, Bytes};
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1v15::Pkcs1v15Sign, traits::PublicKeyParts};
use sha2::{Digest as _, Sha256};
use rand::rngs::OsRng;
use secp256k1::{Secp256k1, SecretKey, Message};
use alloy_rlp::{Encodable, Header};
use bytes::BytesMut;

use leviathan::structs::{BlockHeader, Transaction, VersionId};
use leviathan::world_state::{Account, WorldState};
use leviathan::leviathan::LEVIATHAN;
use my_trait::leviathan_trait::{TransactionExecution, State};

/// イエローペーパー Appendix F に基づくトランザクション署名関数
fn sign_tx_properly(
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
    Header { list: true, payload_length }.encode(&mut out);

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

fn main() {
    let _ = tracing_subscriber::fmt::init();
    let version = VersionId::Petersburg;
    let mut state = WorldState::new();
    let mut leviathan = LEVIATHAN::new(version);
    let secp = Secp256k1::new();

    // ---------------------------------------------------------
    // 1. 送信者 (EOA) の準備（秘密鍵からアドレスを導出）
    // ---------------------------------------------------------
    let secret_key = SecretKey::from_slice(&hex!("45cd63531c3c97355b9275e7a9e6323c2a937a07011d8825e36873c907b29a28")).unwrap();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let serialized_pub = public_key.serialize_uncompressed();
    let pub_hash = keccak256(&serialized_pub[1..65]);
    let sender_addr = Address::from_slice(&pub_hash[12..32]); // 正しい導出方法

    let mut sender_acc = Account::new();
    sender_acc.balance = uint!(100_000_000_000_000_000_000_U256); // 100 ETH
    state.add_account(&sender_addr, sender_acc);

    // ---------------------------------------------------------
    // 2. Solidityコントラクトの配置 (0x88)
    // ---------------------------------------------------------
    let contract_addr = Address::repeat_byte(0x88);
    let mut contract_acc = Account::new();
    // solc --bin-runtime --evm-version petersburg の結果
    let runtime_code = hex::decode("608060405234801561001057600080fd5b5060043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff1680631f0e4b4414610046575b600080fd5b6100666004803603608081101561005c57600080fd5b5080359060208101359060408101359060600135610080565b604051808215151515815260200191505060405180910390f3").expect("Invalid Bytecode");
    contract_acc.code = runtime_code;
    state.add_account(&contract_addr, contract_acc);

    // ---------------------------------------------------------
    // 3. RSAデータの準備 (マイナンバーシミュレーション)
    // ---------------------------------------------------------
    let priv_key = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
    let pub_key = RsaPublicKey::from(&priv_key);
    let msg_hash = keccak256("Leviathan Vote");
    let signature = priv_key.sign(Pkcs1v15Sign::new::<Sha256>(), msg_hash.as_slice()).unwrap();

    // ---------------------------------------------------------
    // 4. Calldataの構築（Solidity ABIエンコードをシミュレート）
    // ---------------------------------------------------------
    let selector = hex!("1f0e4b44");
    let calldata = [
        selector.as_slice(),
        &signature,
        &pub_key.n().to_bytes_be(),
        msg_hash.as_slice(),
        &pub_key.e().to_bytes_be()
    ].concat();

    // ---------------------------------------------------------
    // 5. トランザクションの構築と正当な署名
    // ---------------------------------------------------------
    let t_nonce = 0u64;
    let t_price = uint!(1_U256);
    let t_gas_limit = uint!(1_000_000_U256);
    let t_value = U256::ZERO;

    let (v, r, s) = sign_tx_properly(
        U256::from(t_nonce),
        t_price,
        t_gas_limit,
        Some(contract_addr),
        t_value,
        &calldata,
        &secret_key
    );

    let transaction = Transaction {
        data: calldata,
        t_to: Some(contract_addr),
        t_gas_limit,
        t_price,
        t_value,
        t_nonce: t_nonce as usize,
        t_w: v,
        t_r: r,
        t_s: s,
    };

    // ---------------------------------------------------------
    // 6. 実行
    // ---------------------------------------------------------
    let block = BlockHeader {
        h_beneficiary: Address::repeat_byte(0xfe),
        h_timestamp: uint!(1600000000_U256),
        h_number: uint!(1_U256),
        h_prevrandao: U256::ZERO,
        h_gaslimit: uint!(30_000_000_U256),
        h_basefee: U256::ZERO,
    };

    println!("Starting EVM Execution via Solidity with VALID Signature...");
    match leviathan.execution(&mut state, transaction, &block) {
        Ok((gas, _)) => println!("✅ Success! Remaining Gas: {}", gas),
        Err((gas, _)) => println!("❌ Reverted. Gas consumed: {}", gas),
    }
}
