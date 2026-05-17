use alloy_primitives::{B256, hex};
use clap::{Parser, Subcommand};
use rand::rngs::OsRng;
use rsa::{RsaPrivateKey, pkcs1v15::Pkcs1v15Sign, traits::PublicKeyParts};
use sha2::{Digest, Sha256};
use std::process::Command;

// ※ lib.rs (leviathan_v2) で定義されている前提のモジュールを読み込みます
// 環境に合わせてクレート名（leviathan_v2）は適宜変更してください
use leviathan_v2::zk_prover::ZkVotePayload;

#[derive(Parser)]
#[command(name = "voter_cli", version = "1.0", about = "無人市役所: オフチェーン暗号ペイロード生成ツール")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// [Phase 1] 登録用のRSA署名とコミットメントを生成する
    Register {
        #[arg(short, long)]
        secret: String,
        #[arg(short, long)]
        nullifier: String,
    },
    /// [Phase 2] 投票用のZK-SNARKs証明を生成する
    Vote {
        #[arg(short, long)]
        secret: String,
        #[arg(short, long)]
        nullifier: String,
        #[arg(short, long)]
        choice: String,
        #[arg(long, help = "現在のMerkle Root (16進数)")]
        root: String,
        #[arg(long, help = "自分が登録されたインデックス番号")]
        index: usize,
        #[arg(long, help = "全登録者のコミットメント (カンマ区切り)")]
        all_commitments: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Register { secret, nullifier } => {
            println!("⚙️  Commitment (マイナンバー) と RSA鍵ペアを生成中...\n");

            // 1. circomのJSを叩いてCommitmentを生成
            let output = Command::new("node")
                .current_dir("circom")
                .arg("generate_commitment.js")
                .arg(secret)
                .arg(nullifier)
                .output()
                .expect("generate_commitment.js の実行に失敗しました");
            
            let leaf_hex = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let commitment: B256 = leaf_hex.parse().expect("Invalid commitment format");

            // 2. RSA 2048 鍵ペアの生成と署名
            let mut rng = OsRng;
            let rsa_private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
            let hashed_message = Sha256::digest(commitment.0);
            let signature = rsa_private_key
                .sign(Pkcs1v15Sign::new::<Sha256>(), &hashed_message)
                .unwrap();

            let pub_key_n = rsa_private_key.to_public_key().n().to_bytes_be();
            let pub_key_e = rsa_private_key.to_public_key().e().to_bytes_be();

            let modulus_hex = format!("0x{}", hex::encode(pub_key_n));
            let exponent_hex = format!("0x{}", hex::encode(pub_key_e));
            let signature_hex = format!("0x{}", hex::encode(signature));
            let commitment_hex = format!("0x{}", hex::encode(commitment.0));

            println!("✅ 生成完了！以下のコマンドをコピーして窓口に提出（送信）してください:\n");
            println!(
                "cast send <REGISTRY_ADDRESS> \"register(bytes,bytes,bytes,bytes32)\" \\\n  {} \\\n  {} \\\n  {} \\\n  {} \\\n  --rpc-url http://127.0.0.1:8545 --private-key <YOUR_PK> --legacy\n",
                modulus_hex, exponent_hex, signature_hex, commitment_hex
            );
        }
        Commands::Vote { secret, nullifier, choice, root, index, all_commitments } => {
            println!("⚙️  ゼロ知識証明 (ZK-SNARKs) を生成中...\n");

            let root_hex = root.trim_start_matches("0x").to_string();

            // 1. generate_input.js を実行
            let status = Command::new("node")
                .current_dir("circom")
                .arg("generate_input.js")
                .arg(&root_hex)
                .arg(index.to_string())
                .arg(secret)
                .arg(nullifier)
                .arg(choice)
                .arg(all_commitments)
                .status()
                .expect("Failed to execute generate_input.js");
            assert!(status.success(), "generate_input.js failed");

            // 2. snarkjs で proof を生成
            let snark_status = Command::new("snarkjs")
                .current_dir("circom")
                .args([
                    "groth16",
                    "fullprove",
                    "input.json",
                    "voting_js/voting.wasm",
                    "voting_final.zkey",
                    "proof.json",
                    "public.json",
                ])
                .status()
                .expect("Failed to execute snarkjs");
            assert!(snark_status.success(), "snarkjs fullprove failed");

            // 3. ZkVotePayload を読み込み
            let payload = ZkVotePayload::load_from_snarkjs("circom/proof.json", "circom/public.json");

            let proof_hex = format!("0x{}", hex::encode(&payload.proof_bytes));
            let nullifier_hash_hex = format!("0x{}", hex::encode(payload.nullifier_hash));
            let root_hex_formatted = format!("0x{}", hex::encode(payload.commitment)); // ※既存コードに合わせる

            println!("\n✅ 証明生成完了！以下のコマンドをコピーして投票箱に投函してください:\n");
            println!(
                "cast send <VOTING_ADDRESS> \"castVote(bytes,bytes32,bytes32,uint256)\" \\\n  {} \\\n  {} \\\n  {} \\\n  {} \\\n  --rpc-url http://127.0.0.1:8545 --private-key <YOUR_PK> --legacy\n",
                proof_hex, nullifier_hash_hex, root_hex_formatted, payload.vote_choice
            );
        }
    }
}
