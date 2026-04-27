use alloy_primitives::{B256, Bytes, U256};
use num_bigint::BigUint;
use serde_json::Value;
use std::fs;
use std::str::FromStr;

pub struct ZkVotePayload {
    pub proof_bytes: Bytes,
    pub commitment: B256,
    pub nullifier_hash: B256,
    pub vote_choice: U256,
}

impl ZkVotePayload {
    /// SnarkJS の出力ファイルから EVM 向けの Payload を自動生成する
    pub fn load_from_snarkjs(proof_path: &str, public_path: &str) -> Self {
        let proof_str = fs::read_to_string(proof_path).expect("Failed to read proof.json");
        let proof_json: Value = serde_json::from_str(&proof_str).expect("Invalid proof JSON");

        let pub_str = fs::read_to_string(public_path).expect("Failed to read public.json");
        let pub_json: Value = serde_json::from_str(&pub_str).expect("Invalid public JSON");

        // 10進数文字列を 32バイト(BigEndian) にゼロパディングするヘルパー関数
        let parse_32 = |val: &Value| -> Vec<u8> {
            let num_str = val.as_str().expect("Expected string in JSON");
            let num = BigUint::from_str(num_str).expect("Failed to parse BigUint");
            let bytes = num.to_bytes_be();
            let mut padded = vec![0u8; 32];
            let start = 32 - bytes.len();
            padded[start..].copy_from_slice(&bytes);
            padded
        };

        let mut proof_bytes = Vec::new();
        
        // 1. Point A (X, Y)
        proof_bytes.extend(parse_32(&proof_json["pi_a"][0]));
        proof_bytes.extend(parse_32(&proof_json["pi_a"][1]));
        
        // 2. Point B (Im, Re) - EVMプレコンパイル標準に合わせた順序
        proof_bytes.extend(parse_32(&proof_json["pi_b"][0][1])); // X_im
        proof_bytes.extend(parse_32(&proof_json["pi_b"][0][0])); // X_re
        proof_bytes.extend(parse_32(&proof_json["pi_b"][1][1])); // Y_im
        proof_bytes.extend(parse_32(&proof_json["pi_b"][1][0])); // Y_re
        
        // 3. Point C (X, Y)
        proof_bytes.extend(parse_32(&proof_json["pi_c"][0]));
        proof_bytes.extend(parse_32(&proof_json["pi_c"][1]));

        // 公開入力のパース (Circomの定義順: [commitment, nullifierHash, voteChoice])
        let commitment = B256::from_slice(&parse_32(&pub_json[0]));
        let nullifier_hash = B256::from_slice(&parse_32(&pub_json[1]));
        let vote_choice = U256::from_str(pub_json[2].as_str().unwrap()).unwrap();

        Self {
            proof_bytes: Bytes::from(proof_bytes),
            commitment,
            nullifier_hash,
            vote_choice,
        }
    }
}
