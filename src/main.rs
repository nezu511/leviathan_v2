// 🌟 main.rs には `pub mod ...` を絶対に書かない！（すべて lib.rs に任せる）

use alloy_primitives::{Address, B256, Bytes, U256, hex, keccak256, uint};
use alloy_sol_types::{SolCall, sol};
use rand::rngs::OsRng;
use rsa::{RsaPrivateKey, pkcs1v15::Pkcs1v15Sign, traits::PublicKeyParts};
use secp256k1::{Secp256k1, SecretKey};
use sha2::{Digest, Sha256}; // 🌟 Digest を追加！

// 🌟 すべての機能を自分自身のライブラリ (leviathan_v2) からインポートする
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::VersionId;
use leviathan_v2::leviathan::world_state::{Account, WorldState};
use leviathan_v2::my_trait::leviathan_trait::State;
use leviathan_v2::solidity_utils::{call_contract, deploy_contract};

// Solidityのインターフェースを定義
sol! {
    function register(
        bytes memory modulus,
        bytes memory exponent,
        bytes memory signature,
        bytes memory message,
        bytes32 commitment
    );
}

fn main() {
    let _ = tracing_subscriber::fmt::init();
    let mut state = WorldState::new();
    let mut leviathan = LEVIATHAN::new(VersionId::Petersburg);

    let secret_key = SecretKey::from_byte_array(hex!(
        "45cd63531c3c97355b9275e7a9e6323c2a937a07011d8825e36873c907b29a28"
    ))
    .unwrap();
    let gas_price = uint!(1_U256);
    let gas_limit = uint!(30_000_000_U256);

    //送信者のアドレスを生成
    let secp = Secp256k1::new();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let pub_hash = keccak256(&public_key.serialize_uncompressed()[1..65]);
    let sender_addr = Address::from_slice(&pub_hash[12..32]);

    state.add_account(&sender_addr, Account::new());
    state.add_balance(&sender_addr, U256::MAX);

    // 1. コントラクトのデプロイ
    println!("Deploying IdentityRegistry");
    let contract_addr = deploy_contract(
        &mut leviathan,
        &mut state,
        &secret_key,
        "solidity/out/IdentityRegistry.bin",
        U256::ZERO,
        gas_price,
        gas_limit,
    )
    .expect("Deployment failed");
    println!("✅ Deployed at: {:?}", contract_addr);

    // 2. RSA署名データの準備
    println!("Generating RSA Keys and Signature...");
    let mut rng = OsRng;
    let rsa_private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let rsa_public_key = rsa_private_key.to_public_key();

    let message = b"I want to register my commitment to Leviathan";
    let hashed_message = Sha256::digest(message);
    let signature = rsa_private_key
        .sign(Pkcs1v15Sign::new::<Sha256>(), &hashed_message)
        .unwrap();

    let pub_key_n = rsa_public_key.n().to_bytes_be();
    let pub_key_e = rsa_public_key.e().to_bytes_be();

    // ZK回路側で生成した想定の仮想Commitment
    let my_commitment = B256::repeat_byte(0x77);

    // 3. alloy を使ったABIエンコード
    println!("Encoding payload");
    let call_data = registerCall {
        modulus: Bytes::from(pub_key_n),
        exponent: Bytes::from(pub_key_e),
        signature: Bytes::from(signature),
        message: Bytes::from(message.to_vec()),
        commitment: my_commitment,
    }
    .abi_encode();

    // 4. 実行
    println!("Sending register transaction to EVM");
    call_contract(
        &mut leviathan,
        &mut state,
        &secret_key,
        contract_addr,
        call_data,
        U256::ZERO,
        gas_price,
        gas_limit,
    )
    .expect("Register Call Failed");

    sol! {
        function isRegistered(bytes32 commitment) external view returns (bool);
    }

    let check_data = isRegisteredCall {
        commitment: my_commitment,
    }
    .abi_encode();
    let result_logs = call_contract(
        &mut leviathan,
        &mut state,
        &secret_key,
        contract_addr,
        check_data,
        U256::ZERO,
        gas_price,
        gas_limit,
    )
    .unwrap();

    // 先ほどLEVIATHAN構造体に追加した return_data バッファから結果を読み取る
    let is_reg = leviathan.return_data[31] == 1;
    println!("Is commitment registered? \n{}", is_reg);
}
