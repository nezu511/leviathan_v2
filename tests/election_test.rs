// 🌟 main.rs には `pub mod ...` を絶対に書かない！（すべて lib.rs に任せる）

use alloy_primitives::{Address, Bytes, U256, hex, keccak256, uint};
use alloy_sol_types::{SolCall, sol};
use rand::rngs::OsRng;
use rsa::{RsaPrivateKey, pkcs1v15::Pkcs1v15Sign, traits::PublicKeyParts};
use secp256k1::{Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::VersionId;
use leviathan_v2::leviathan::world_state::{Account, WorldState};
use leviathan_v2::my_trait::leviathan_trait::State;
use leviathan_v2::solidity_utils::{call_contract, deploy_contract, deploy_contract_raw};
use leviathan_v2::zk_prover::ZkVotePayload;

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

    function isRegistered(bytes32 commitment) external view returns (bool);
}

// =====================================================================
// 選挙のメインストーリー（無人市役所の業務フロー）
// =====================================================================
#[test]
fn test_election_e2e() {
    let _ = tracing_subscriber::fmt::init();

    // 1. ブロックチェーン環境の起動
    let (mut state, mut leviathan, secret_key, gas_price, gas_limit) = setup_evm();

    // 2. スマホで生成された「身分証（コミットメント）」の読み込み
    let payload = ZkVotePayload::load_from_snarkjs("circom/proof.json", "circom/public.json");

    // 3. Phase 1: 市役所（名簿）の設立と、RSA認証による有権者登録
    println!("--- Phase 1: Voter Registration ---");
    let registry_addr = deploy_identity_registry(&mut leviathan, &mut state, &secret_key, gas_price, gas_limit);
    register_voter(&mut leviathan, &mut state, &secret_key, registry_addr, &payload, gas_price, gas_limit);

    // =========================================================================
    // 次のステップ: ここで最新のRootを取得し、Proofを再生成する処理が入ります
    // println!("--- Phase 1.5: Auto-Generating Fresh ZK Proof ---");
    // let fresh_payload = regenerate_proof_with_official_root(&state, registry_addr);
    // =========================================================================

    // 4. Phase 2: 投票所の設立と匿名投票
    println!("--- Phase 2: Anonymous ZK Voting ---");
    let voting_addr = deploy_voting_contract(&mut leviathan, &mut state, &secret_key, registry_addr, gas_price, gas_limit);
    
    // ※ 現在は古いRootを使っているため、意図的にRevert(防衛成功)します
    cast_anonymous_vote(&mut leviathan, &mut state, &secret_key, voting_addr, &payload, gas_price, gas_limit);

    // 5. 選挙結果の集計
    check_election_results(&mut state, voting_addr, &payload);
}


// =====================================================================
// 抽出したヘルパーメソッド群（インフラ処理）
// =====================================================================

fn setup_evm() -> (WorldState, LEVIATHAN, SecretKey, U256, U256) {
    let mut state = WorldState::new();
    let leviathan = LEVIATHAN::new(VersionId::Petersburg);

    let secret_key = SecretKey::from_byte_array(hex!("45cd63531c3c97355b9275e7a9e6323c2a937a07011d8825e36873c907b29a28")).unwrap();
    let gas_price = uint!(1_U256);
    let gas_limit = uint!(30_000_000_U256);

    let secp = Secp256k1::new();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let pub_hash = keccak256(&public_key.serialize_uncompressed()[1..65]);
    let sender_addr = Address::from_slice(&pub_hash[12..32]);

    state.add_account(&sender_addr, Account::new());
    state.add_balance(&sender_addr, U256::MAX);

    (state, leviathan, secret_key, gas_price, gas_limit)
}

fn deploy_identity_registry(
    leviathan: &mut LEVIATHAN, state: &mut WorldState, secret_key: &SecretKey, gas_price: U256, gas_limit: U256
) -> Address {
    println!("Deploying IdentityRegistry...");
    let contract_addr = deploy_contract(
        leviathan, state, secret_key, "solidity/out/IdentityRegistry.bin", U256::ZERO, gas_price, gas_limit
    ).expect("Deployment failed");
    println!("✅ Deployed at: {:?}", contract_addr);
    contract_addr
}

fn register_voter(
    leviathan: &mut LEVIATHAN, state: &mut WorldState, secret_key: &SecretKey, 
    registry_addr: Address, payload: &ZkVotePayload, gas_price: U256, gas_limit: U256
) {
    println!("Generating RSA Keys and Signature...");
    let mut rng = OsRng;
    let rsa_private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let message = b"I want to register my commitment to Leviathan";
    let hashed_message = Sha256::digest(message);
    let signature = rsa_private_key.sign(Pkcs1v15Sign::new::<Sha256>(), &hashed_message).unwrap();

    let pub_key_n = rsa_private_key.to_public_key().n().to_bytes_be();
    let pub_key_e = rsa_private_key.to_public_key().e().to_bytes_be();

    let call_data = registerCall {
        modulus: Bytes::from(pub_key_n),
        exponent: Bytes::from(pub_key_e),
        signature: Bytes::from(signature),
        message: Bytes::from(message.to_vec()),
        commitment: payload.commitment,
    }.abi_encode();

    println!("Sending register transaction to EVM...");
    call_contract(leviathan, state, secret_key, registry_addr, call_data, U256::ZERO, gas_price, gas_limit)
        .expect("Register Call Failed");

    let check_data = isRegisteredCall { commitment: payload.commitment }.abi_encode();
    let _ = call_contract(leviathan, state, secret_key, registry_addr, check_data, U256::ZERO, gas_price, gas_limit).unwrap();
    
    let is_reg = leviathan.return_data[31] == 1;
    println!("Is commitment registered? {}", is_reg);
}

fn deploy_voting_contract(
    leviathan: &mut LEVIATHAN, state: &mut WorldState, secret_key: &SecretKey, 
    registry_addr: Address, gas_price: U256, gas_limit: U256
) -> Address {
    println!("Deploying VK_Data...");
    let vk_init_code = std::fs::read("solidity/out/VK_Data.bin").unwrap();
    let vk_addr = deploy_contract_raw(leviathan, state, secret_key, vk_init_code, U256::ZERO, gas_price, gas_limit).unwrap();
    println!("✅ VK_Data deployed at: {:?}", vk_addr);

    println!("Deploying Voting Contract...");
    let mut v_init = hex::decode(std::fs::read_to_string("solidity/out/Voting.bin").unwrap().trim().trim_start_matches("0x")).unwrap();
    
    let mut args = Vec::new();
    args.extend_from_slice(&[0u8; 12]);
    args.extend_from_slice(vk_addr.as_slice());
    args.extend_from_slice(&[0u8; 12]);
    args.extend_from_slice(registry_addr.as_slice());
    v_init.extend(args);

    let v_addr = deploy_contract_raw(leviathan, state, secret_key, v_init, U256::ZERO, gas_price, gas_limit).unwrap();
    println!("✅ Voting Contract deployed at: {:?}", v_addr);
    v_addr
}

fn cast_anonymous_vote(
    leviathan: &mut LEVIATHAN, state: &mut WorldState, secret_key: &SecretKey, 
    voting_addr: Address, payload: &ZkVotePayload, gas_price: U256, gas_limit: U256
) {
    let vote_payload = castVoteCall {
        proof: payload.proof_bytes.clone(),
        nullifierHash: payload.nullifier_hash,
        root: payload.commitment, // 現在は固定値のコミットメントを入れている
        voteChoice: payload.vote_choice,
    }.abi_encode();

    println!("Sending ZK Vote transaction to EVM...");
    let _ = call_contract(leviathan, state, secret_key, voting_addr, vote_payload, U256::ZERO, gas_price, gas_limit);
}

fn check_election_results(state: &mut WorldState, voting_addr: Address, payload: &ZkVotePayload) {
    println!("--- Final Check: Vote Count ---");
    let vote_choice = uint!(1_U256);

    let mut storage_key_src = [0u8; 64];
    storage_key_src[0..32].copy_from_slice(&vote_choice.to_be_bytes::<32>());
    storage_key_src[32..64].copy_from_slice(&uint!(2_U256).to_be_bytes::<32>());
    
    let storage_key_u256: U256 = keccak256(storage_key_src).into();
    let vote_count = state.get_storage_value(&voting_addr, &storage_key_u256).unwrap_or(U256::ZERO);
    println!("Votes for choice 1: {:?}", vote_count);

    let mut null_key_src = [0u8; 64];
    null_key_src[0..32].copy_from_slice(&payload.nullifier_hash.0);
    null_key_src[32..64].copy_from_slice(&uint!(1_U256).to_be_bytes::<32>());
    
    let is_spent = state.get_storage_value(&voting_addr, &keccak256(null_key_src).into()).unwrap_or(U256::ZERO);
    println!("Is nullifier spent? (1 = true): {:?}", is_spent);
}
