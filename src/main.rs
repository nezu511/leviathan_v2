#![allow(dead_code)]
pub mod evm;
pub mod leviathan;
pub mod my_trait;
pub mod test;

use alloy_primitives::{Address, U256, hex, keccak256, Bytes, uint};
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1v15::Pkcs1v15Sign, traits::PublicKeyParts};
use sha2::{Digest, Sha256};
use rand::rngs::OsRng;
use secp256k1::{Secp256k1, SecretKey, Message};

use leviathan::structs::{BlockHeader, Transaction, VersionId};
use leviathan::world_state::{Account, WorldState};
use leviathan::leviathan::LEVIATHAN;
use my_trait::leviathan_trait::{TransactionExecution, State};

fn main() {
    let _ = tracing_subscriber::fmt::init();
    let version = VersionId::Petersburg;
    let mut state = WorldState::new();
    let mut leviathan = LEVIATHAN::new(version);

    // ---------------------------------------------------------
    // 1. 送信者 (EOA) の準備
    // ---------------------------------------------------------
    let sender_addr = hex!("6388A962E3C0F5953761E0D11111111111111111").into();

    let mut sender_acc = Account::new();
    sender_acc.balance = uint!(100_000_000_000_000_000_000_U256); // 100 ETH
    state.add_account(&sender_addr, sender_acc);

    // ---------------------------------------------------------
    // 2. Solidityコントラクトの配置 (0x88)
    // ---------------------------------------------------------
    let contract_addr = Address::repeat_byte(0x88);
    let mut contract_acc = Account::new();
    // solcの出力をここに貼り付け
    let runtime_code = hex::decode("608060405234801561001057600080fd5b506004361061").expect("Invalid Bytecode");
    contract_acc.code = runtime_code;
    state.add_account(&contract_addr, contract_acc);

    // ---------------------------------------------------------
    // 3. RSAデータの準備 (マイナンバーシミュレーション)
    // ---------------------------------------------------------
    let priv_key = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
    let pub_key = RsaPublicKey::from(&priv_key);
    let msg_hash = keccak256("Leviathan Vote");
    // .as_slice() で FixedBytes を &[u8] に変換
    let signature = priv_key.sign(Pkcs1v15Sign::new::<Sha256>(), msg_hash.as_slice()).unwrap();

    // ---------------------------------------------------------
    // 4. Calldataの構築
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
    // 5. トランザクションの構築
    // ---------------------------------------------------------
    let transaction = Transaction {
        data: calldata,
        t_to: Some(contract_addr),
        t_gas_limit: uint!(1_000_000_U256),
        t_price: uint!(1_U256),
        t_value: U256::ZERO,
        t_nonce: 0,
        t_w: uint!(27_U256),
        t_r: U256::ZERO,
        t_s: U256::ZERO,
    };

    // ---------------------------------------------------------
    // 6. 実行 (Defaultを使わずに全フィールドを埋める)
    // ---------------------------------------------------------
    let block = BlockHeader {
        h_beneficiary: Address::repeat_byte(0xfe),
        h_timestamp: uint!(1600000000_U256),
        h_number: uint!(1_U256),
        h_prevrandao: U256::ZERO,
        h_gaslimit: uint!(30_000_000_U256),
        h_basefee: U256::ZERO,
    };

    println!("Starting EVM Execution via Solidity...");
    match leviathan.execution(&mut state, transaction, &block) {
        Ok((gas, _)) => println!("✅ Success! Remaining Gas: {}", gas),
        Err((gas, _)) => println!("❌ Reverted. Gas consumed: {}", gas),
    }
}
