#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction};
use crate::evm::evm::EVM;
use sha3::{Keccak256, Digest};
use rlp::RlpStream;
use secp256k1::{
    ecdsa::{RecoverableSignature, RecoveryId},
    Message, Secp256k1,
};

#[derive(Debug,Clone)]
pub enum Action {
    Sstorage (Address,U256, U256),         //Address, pre_value, Key
    Send_eth (Address, Address, U256),       //from, to, eth
    Add_nonce (Address),
    Store_code (Address, Vec<u8>),
    Account_creation (Address),
    Child_evm (usize),
}


pub struct LEVIATHAN (Vec<Action>);

impl TransactionExecution for LEVIATHAN {
     fn execution(state: &mut WorldState, transaction:Transaction) -> Result<(U256, Vec<Log>, bool),(U256, Vec<Log>, bool)> {
        //=======ステップ1===========
        //初期ガスの計算
        let base_gas = U256::from(21000);  //基本料金
        let mut data_gas = U256::ZERO;
        let mut index = 0;
        while index < transaction.data.len() {  //データに関するガス
            if transaction.data[index] == 0 {
                data_gas = data_gas.saturating_add(U256::from(4));
            }else{
                data_gas = data_gas.saturating_add(U256::from(16));
            }
            index += 1;
        }
        let mut contract_gas = U256::ZERO;
        if transaction.t_to.is_none() {     //コントラクト作成追加費
            contract_gas = contract_gas.saturating_add(U256::from(32000));
            let words = U256::from(transaction.data.len()).saturating_add(U256::from(31)) / U256::from(32);
            let word_gas = words.saturating_mul(U256::from(2));
            contract_gas = contract_gas.saturating_add(word_gas);
        }

        let all_gas = base_gas + data_gas + contract_gas;

        //事前支払いコスト
        let max_cost = transaction.t_gas_limit.saturating_mul(transaction.t_price) + transaction.t_value;

        //公開鍵がGET!
        let mut stream = RlpStream::new_list(6);

        // 【解決策1】 U256 を RLP 向けにバイト列（先頭のゼロを省略した形式）に変換するヘルパー関数
        let append_u256 = |stream: &mut RlpStream, val: &alloy_primitives::U256| {
            let bytes = val.to_be_bytes::<32>(); // U256を32バイトのビッグエンディアン配列に変換
            let trimmed = match bytes.iter().position(|&b| b != 0) {
                Some(i) => &bytes[i..], // 最初の非ゼロバイト以降を取得
                None => &[],            // 値が 0 の場合は空の配列 (RLP仕様)
            };
        stream.append(&trimmed);
        };

        stream.append(&transaction.t_nonce); // usize はデフォルトで対応しているのでそのままOK
        append_u256(&mut stream, &transaction.t_price);
        append_u256(&mut stream, &transaction.t_gas_limit);

        match &transaction.t_to {
            Some(address) => {
                // 【解決策2】 自作Address構造体の中身の配列([u8; 20])をスライスとして渡す
                stream.append(&address.0.as_slice()); 
            }
            None => {
                stream.append_empty_data();
            }
        }

        append_u256(&mut stream, &transaction.t_value);
        stream.append(&transaction.data);

        let rlp_encoded = stream.out();
        let mut hasher = Keccak256::new();
        hasher.update(&rlp_encoded);
        let tx_hash_bytes: [u8; 32] = hasher.finalize().into();

        // --- 公開鍵のリカバリ部分 ---

        // 【解決策3】 secp256k1 最新版では `from_digest` を使う
        let message = Message::from_digest(tx_hash_bytes);

        // 【解決策4】 U256 -> u64 への変換は `try_into()` を使う
        let t_w_u64: u64 = transaction.t_w.try_into().expect("t_w is too large for u64");
        let v_val = (t_w_u64 - 27) as u8;

        // 【解決策5】 `from_i32` の代わりに `TryFrom::try_from` を使う
        let recovery_id = RecoveryId::try_from(v_val as i32).expect("Invalid v");

        // 【解決策6】 `to_big_endian` の代わりに `to_be_bytes::<32>()` を使う
        let mut sig_bytes = [0u8; 64];
        sig_bytes[0..32].copy_from_slice(&transaction.t_r.to_be_bytes::<32>());
        sig_bytes[32..64].copy_from_slice(&transaction.t_s.to_be_bytes::<32>());

        let signature = RecoverableSignature::from_compact(&sig_bytes, recovery_id).expect("Invalid signature");

        let secp = Secp256k1::verification_only();

        // 【解決策7】 最新版では `&message` ではなく `message` (値渡し) にする
        let public_key = secp.recover_ecdsa(message, &signature)
            .expect("Failed to recover public key");

        // あとは前回のコードと同じようにアドレスを抽出！
        let uncompressed_pubkey = public_key.serialize_uncompressed();
        let pubkey_hash = Keccak256::digest(&uncompressed_pubkey[1..65]);
        let mut sender_address = [0u8; 20];
        sender_address.copy_from_slice(&pubkey_hash[12..32]);

        return Ok((U256::ZERO, Vec::new(), true));







         
     }
}

