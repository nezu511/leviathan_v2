use crate::LeviathanApp;
use alloy_primitives::{Address, TxKind, U256};
use alloy_rlp::{Encodable, Header};
use bytes::BytesMut;
use leviathan_v2::leviathan::structs::{Transaction, VersionId};
use leviathan_v2::my_trait::leviathan_trait::State;
use secp256k1::{
    Message, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use sha3::{Digest, Keccak256};

pub trait Tx_Checker {
    fn validate_transaction(&self, transaction: &Transaction) -> bool;
}

impl Tx_Checker for LeviathanApp {
    fn validate_transaction(&self, transaction: &Transaction) -> bool {
        //=======ステップ1===========
        //【初期ガスの計算】
        let base_gas = U256::from(21000); //基本料金
        let mut data_gas = U256::ZERO;
        let mut index = 0;

        //データに関するガス
        if self.version < VersionId::Istanbul {
            //Istanbul以前
            while index < transaction.data.len() {
                if transaction.data[index] == 0 {
                    data_gas = data_gas.saturating_add(U256::from(4));
                } else {
                    data_gas = data_gas.saturating_add(U256::from(68));
                }
                index += 1;
            }
        } else {
            while index < transaction.data.len() {
                if transaction.data[index] == 0 {
                    data_gas = data_gas.saturating_add(U256::from(4));
                } else {
                    data_gas = data_gas.saturating_add(U256::from(16));
                }
                index += 1;
            }
        }

        let mut contract_gas = U256::ZERO;
        if transaction.t_to.is_create() {
            //コントラクト作成追加費
            if self.version >= VersionId::Homestead {
                //Homestead以降
                contract_gas = contract_gas.saturating_add(U256::from(32000));

                if self.version >= VersionId::Shanghai {
                    //Shanghai以降
                    //Initcodeのサイズに対する従量課金
                    let words = U256::from(transaction.data.len()).saturating_add(U256::from(31))
                        / U256::from(32);
                    let word_gas = words.saturating_mul(U256::from(2));
                    contract_gas = contract_gas.saturating_add(word_gas);
                }
            }
        }
        let all_gas = base_gas + data_gas + contract_gas;
        //【事前支払いコスト】
        let max_cost =
            transaction.t_gas_limit.saturating_mul(transaction.t_price) + transaction.t_value;


        let Ok(t_w_u64) = u64::try_from(transaction.t_w) else {
            tracing::warn!("t_w is too large for u64");
            return false;
        };

        // vの値から「リカバリID」と「Chain ID」を逆算する！
        let v_val = t_w_u64;
        let (recovery_id_u8, chain_id) = if v_val == 27 || v_val == 28 {
            ( (v_val - 27) as u8, None ) // 昔の方式
        } else if v_val >= 35 {
            ( ((v_val - 35) % 2) as u8, Some((v_val - 35) / 2) ) // 最新のEIP-155方式
        } else {
            tracing::warn!("Invalid v value: {}", v_val);
            return false;
        };

        let Ok(recovery_id) = RecoveryId::try_from(recovery_id_u8 as i32) else {
            tracing::warn!("Invalid recovery id");
            return false;
        };

        // --- RLPエンコード (署名検証用のハッシュ生成) ---
        let mut payload_length = 0;
        payload_length += transaction.t_nonce.length();
        payload_length += transaction.t_price.length();
        payload_length += transaction.t_gas_limit.length();

        let to_slice = match &transaction.t_to {
            TxKind::Call(address) => address.0.as_slice(),
            TxKind::Create => &[], // 空のバイト列
        };
        payload_length += to_slice.length();
        payload_length += transaction.t_value.length();
        payload_length += transaction.data.length(); // Bytesに変更したのでコレでOK！

        // ★ EIP-155の場合は、ハッシュ化するデータに ChainID, 0, 0 を追加する！
        if let Some(cid) = chain_id {
            payload_length += cid.length();
            payload_length += 0u64.length(); // r の代わり (0)
            payload_length += 0u64.length(); // s の代わり (0)
        }

        let mut out = BytesMut::with_capacity(payload_length + 10);
        Header {
            list: true,
            payload_length,
        }
        .encode(&mut out);

        transaction.t_nonce.encode(&mut out);
        transaction.t_price.encode(&mut out);
        transaction.t_gas_limit.encode(&mut out);
        to_slice.encode(&mut out);
        transaction.t_value.encode(&mut out);
        transaction.data.encode(&mut out);

        // ★ 追加データもエンコードして書き込む
        if let Some(cid) = chain_id {
            cid.encode(&mut out);
            0u64.encode(&mut out);
            0u64.encode(&mut out);
        }

        let rlp_encoded = out.freeze();
        
        // 4. Keccak256でハッシュ化して32バイトのh(T)を得る
        let mut hasher = Keccak256::new();
        hasher.update(&rlp_encoded);
        let tx_hash_bytes: [u8; 32] = hasher.finalize().into();
        
        // --- 公開鍵のリカバリ部分 ---
        let message = Message::from_digest(tx_hash_bytes);

        // 【解決策6】 `to_big_endian` の代わりに `to_be_bytes::<32>()` を使う
        let mut sig_bytes = [0u8; 64];
        sig_bytes[0..32].copy_from_slice(&transaction.t_r.to_be_bytes::<32>());
        sig_bytes[32..64].copy_from_slice(&transaction.t_s.to_be_bytes::<32>());
        let Ok(signature) = RecoverableSignature::from_compact(&sig_bytes, recovery_id) else {
            tracing::warn!("Invalid signature");
            return false;
        };
        let secp = Secp256k1::verification_only();
        // 【解決策7】 最新版では `&message` ではなく `message` (値渡し) にする
        let Ok(public_key) = secp.recover_ecdsa(message, &signature) else {
            tracing::warn!("Failed to recover public key");
            return false;
        };
        // あとは前回のコードと同じようにアドレスを抽出！
        let uncompressed_pubkey = public_key.serialize_uncompressed();
        let pubkey_hash = Keccak256::digest(&uncompressed_pubkey[1..65]);
        let mut sender_address = [0u8; 20];
        sender_address.copy_from_slice(&pubkey_hash[12..32]);
        let sender_address = Address::new(sender_address);

        //self.stateをロックして，中身のstateを取り出す
        let mut state = self.state.write().unwrap();

        //Nonceの整合性
        let Some(sender_nonce) = state.get_nonce(&sender_address) else {
            tracing::warn!("送信者のアカウントが見つからない");
            return false;
        };
        if sender_nonce as usize != transaction.t_nonce {
            tracing::warn!("nonceが不一致");
            return false;
        }

        //Codeの不在
        let sender_code = state.get_code(&sender_address).unwrap();
        if !sender_code.is_empty() {
            tracing::warn!("送信者のアカウントにコントラクトコードがデプロイされている");
            return false;
        }

        //ガスリミットの妥当性
        let gas_limit = transaction.t_gas_limit;
        if gas_limit < all_gas {
            tracing::warn!("初期ガスが指定されたガスリミットを超えている");
            return false;
        }

        //残高の妥当性
        let sender_balance = state.get_balance(&sender_address).unwrap();
        if sender_balance < max_cost {
            tracing::warn!("送信者の残高が事前支払いコストを満たしていない");
            return false;
        }

        //Initコードが49152バイト以下
        if self.version >= VersionId::Shanghai {
            //Shanghai以降
            if transaction.t_to.is_create() && transaction.data.len() > 49152 {
                tracing::warn!("Initコードが49152バイトを超えている");
                return false;
            }
        }

        true
    }
}
