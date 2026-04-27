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
use leviathan_v2::solidity_utils::{call_contract, deploy_contract, deploy_contract_raw};
use leviathan_v2::zk_prover::ZkVotePayload;

// Solidityのインターフェースを定義
sol! {
    function register(
        bytes memory modulus,
        bytes memory exponent,
        bytes memory signature,
        bytes memory message,
        bytes32 commitment
    );

    function castVote(
        bytes memory proof,
        bytes32 nullifierHash,
        bytes32 root,
        uint256 voteChoice
    ) external;
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

    //=========================================
    //              Phase1: RSA
    //=========================================

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

    let payload = ZkVotePayload::load_from_snarkjs("circom/proof.json", "circom/public.json");
    // 3. alloy を使ったABIエンコード
    println!("Encoding payload");
    let call_data = registerCall {
        modulus: Bytes::from(pub_key_n),
        exponent: Bytes::from(pub_key_e),
        signature: Bytes::from(signature),
        message: Bytes::from(message.to_vec()),
        commitment: payload.commitment,
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
        commitment: payload.commitment,
    }
    .abi_encode();
    println!("Is commitment registered ? (call isRegistered) ...");
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
    println!("{}", is_reg);

    //=========================================
    //              Phase2: ZK
    //=========================================
    //
    println!("--- Step 3: Phase 2 - Anonymous ZK Voting ---");

    //1. VK データを「バイナリ」として読み込み、rawデプロイする
    println!("Deploying VK_Data...");

    // std::fs::read を使って、生のバイト列として読み込む
    let vk_init_code =
        std::fs::read("solidity/out/VK_Data.bin").expect("Failed to read binary VK_Data.bin");

    // deploy_contract ではなく、deploy_contract_raw を使う
    let vk_addr = leviathan_v2::solidity_utils::deploy_contract_raw(
        &mut leviathan,
        &mut state,
        &secret_key,
        vk_init_code, // 読み込んだバイナリをそのまま渡す
        U256::ZERO,
        gas_price,
        gas_limit,
    )
    .expect("VK Deployment failed");

    println!("✅ VK_Data deployed at: {:?}", vk_addr);

    // 2. Voting コントラクトのデプロイ (引数として vk_addr を渡す)
    println!("Deploying Voting Contract...");

    let mut v_init = hex::decode(
        std::fs::read_to_string("solidity/out/Voting.bin")
            .unwrap()
            .trim()
            .trim_start_matches("0x"),
    )
    .unwrap();
    let mut args = vec![0u8; 12]; // 32バイトに合わせるパディング
    args.extend_from_slice(vk_addr.as_slice());
    v_init.extend(args);

    let v_addr = leviathan_v2::solidity_utils::deploy_contract_raw(
        &mut leviathan,
        &mut state,
        &secret_key,
        v_init,
        U256::ZERO,
        gas_price,
        gas_limit,
    )
    .expect("Voting Deploy Failed");


    let vote_payload = castVoteCall {
        proof: payload.proof_bytes,
        nullifierHash: payload.nullifier_hash,
        root: payload.commitment,
        voteChoice: payload.vote_choice,
    }
    .abi_encode();

    println!("Sending ZK Vote transaction to EVM...");

    let _ = call_contract(
        &mut leviathan,
        &mut state,
        &secret_key,
        v_addr,
        vote_payload,
        U256::ZERO,
        gas_price,
        gas_limit,
    )
    .expect("ZK Vote Execution Failed");

    // 結果確認

    println!("--- Final Check: Vote Count ---");
    // 投票先の選択肢 1
    let vote_choice = uint!(1_U256);

    // mapping(uint256 => uint256) votes は、スロット 2 にあります
    let mut storage_key_src = [0u8; 64];
    storage_key_src[0..32].copy_from_slice(&vote_choice.to_be_bytes::<32>());
    storage_key_src[32..64].copy_from_slice(&uint!(2_U256).to_be_bytes::<32>());
    let storage_key = keccak256(storage_key_src);

    let storage_key_u256: U256 = storage_key.into();
    let vote_count = state
        .get_storage_value(&v_addr, &storage_key_u256)
        .unwrap_or(U256::ZERO);

    println!("Votes for choice 1: {:?}", vote_count);

    // Nullifierが「使用済み」になっているかもチェック
    let mut null_key_src = [0u8; 64];
    null_key_src[0..32].copy_from_slice(&payload.nullifier_hash.0);
    null_key_src[32..64].copy_from_slice(&uint!(1_U256).to_be_bytes::<32>()); // spentNullifiersはスロット 1
    let null_storage_key = keccak256(null_key_src);
    let is_spent = state
        .get_storage_value(&v_addr, &null_storage_key.into())
        .unwrap_or(U256::ZERO);
    println!("Is nullifier spent? (1 = true): {:?}", is_spent);
}
