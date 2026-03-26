#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution, TransactionChecks};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction};
use crate::evm::evm::EVM;
use sha3::{Keccak256, Digest};
use rlp::RlpStream;
use secp256k1::{
    ecdsa::{RecoverableSignature, RecoveryId},
    Message, Secp256k1,
};

impl TransactionChecks for LEVIATHAN {
     fn transaction_checks(state: &mut WorldState, transaction:&Transaction) -> Result<Address,&'static str> {
        //公開鍵取得
        //1. RlpStreamを使って，6つの要素をもつリストを作成する．
        let mut stream = RlpStream::new_list(6);    
        // U256 を RLP 向けにバイト列（先頭のゼロを省略した形式）に変換するヘルパー関数
        let append_u256 = |stream: &mut RlpStream, val: &alloy_primitives::U256| {
            let bytes = val.to_be_bytes::<32>(); // U256を32バイトのビッグエンディアン配列に変換
            let trimmed = match bytes.iter().position(|&b| b != 0) {
                Some(i) => &bytes[i..], // 最初の非ゼロバイト以降を取得
                None => &[],            // 値が 0 の場合は空の配列 (RLP仕様)
            };
        stream.append(&trimmed);
        };
        //イエローペーパーの定義通りに，順番に要素を追加
        stream.append(&transaction.t_nonce); 
        append_u256(&mut stream, &transaction.t_price);
        append_u256(&mut stream, &transaction.t_gas_limit);
        match &transaction.t_to {
            Some(address) => {
                //自作Address構造体の中身の配列([u8; 20])をスライスとして渡す
                stream.append(&address.0.as_slice()); 
            }
            None => {
                stream.append_empty_data();
            }
        }
        append_u256(&mut stream, &transaction.t_value);
        stream.append(&transaction.data);
        //3. RLPエンコードされたバイト列を取り出す
        let rlp_encoded = stream.out();
        //4. Keccak256でハッシュ化して32バイトのh(T)を得る
        let mut hasher = Keccak256::new();
        hasher.update(&rlp_encoded);
        let tx_hash_bytes: [u8; 32] = hasher.finalize().into();
        // --- 公開鍵のリカバリ部分 ---
        let message = Message::from_digest(tx_hash_bytes);
        let t_w_u64: u64 = transaction.t_w.try_into().map_err(|_|"t_w is too large for u64")?;
        let v_val = (t_w_u64 - 27) as u8;
        // 【解決策5】 `from_i32` の代わりに `TryFrom::try_from` を使う
        let recovery_id = RecoveryId::try_from(v_val as i32).map_err(|_|"Invalid v")?;
        // 【解決策6】 `to_big_endian` の代わりに `to_be_bytes::<32>()` を使う
        let mut sig_bytes = [0u8; 64];
        sig_bytes[0..32].copy_from_slice(&transaction.t_r.to_be_bytes::<32>());
        sig_bytes[32..64].copy_from_slice(&transaction.t_s.to_be_bytes::<32>());
        let signature = RecoverableSignature::from_compact(&sig_bytes, recovery_id).map_err(|_|"Invalid signature")?;
        let secp = Secp256k1::verification_only();
        // 【解決策7】 最新版では `&message` ではなく `message` (値渡し) にする
        let public_key = secp.recover_ecdsa(message, &signature).map_err(|_|"Failed to recover public key")?;
        // あとは前回のコードと同じようにアドレスを抽出！
        let uncompressed_pubkey = public_key.serialize_uncompressed();
        let pubkey_hash = Keccak256::digest(&uncompressed_pubkey[1..65]);
        let mut sender_address = [0u8; 20];
        sender_address.copy_from_slice(&pubkey_hash[12..32]);
        let sender_address = Address::new(sender_address);

        //Nonceの整合性
        let sender_nonce = state.get_nonce(&sender_address);



        return Ok(sender_address);
     }
}




