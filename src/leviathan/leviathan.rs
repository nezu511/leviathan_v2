#![allow(dead_code)]

use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{
    BackupSubstate, BlockHeader, Log, SubState, Transaction, VersionId,
};
use crate::leviathan::world_state::{Account, WorldState, MptAccount};
use crate::my_trait::leviathan_trait::{
    ContractCreation, MessageCall, State, TransactionChecks, TransactionExecution,
};
use alloy_primitives::{U256, hex, keccak256};
use sha3::Digest;
use alloy_rlp::Encodable;
use eth_trie::{EthTrie, Trie};

pub struct LEVIATHAN {
    pub journal: Vec<Action>,
    pub substate_backup: BackupSubstate,
    pub version: VersionId,
}

impl LEVIATHAN {
    pub fn new(version: VersionId) -> Self {
        Self {
            journal: Vec::<Action>::new(),
            substate_backup: BackupSubstate::new(),
            version,
        }
    }

    pub fn merge(&mut self, mut children: LEVIATHAN) {
        self.journal.append(&mut children.journal);
    }
}

impl TransactionExecution for LEVIATHAN {
    fn execution(
        &mut self,
        state: &mut WorldState,
        transaction: Transaction,
        block_header: &BlockHeader,
    ) -> Result<(U256, Vec<Log>), (U256, Vec<Log>)> {
        tracing::info!("version: {:?}", self.version);
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
        if transaction.t_to.is_none() {
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
        //【トランザクションの事前検証】
        let sender_address =
            self.transaction_checks(state, &transaction, &all_gas, &max_cost, block_header);
        if sender_address.is_err() {
            return Err((U256::ZERO, Vec::new()));
        }
        let sender_address = sender_address.unwrap();

        //=======ステップ2===========
        //【Nonceの加算】
        if state.is_empty(&sender_address) {
            return Err((U256::ZERO, Vec::new())); //sender_addressが見つからないのは異常
        }
        state.inc_nonce(&sender_address);
        //【前払いガス代の徴収】
        let gas = state.buy_gas(
            &sender_address,
            transaction.t_gas_limit,
            transaction.t_price,
        );
        //ここからロールバックの起点:ロールバックが起きたらこの状態にする
        let mut substate = SubState::new();

        //gasから初期ガスを引く
        let mut gas = gas.unwrap();
        gas = gas.saturating_sub(all_gas);

        //=======ステップ3===========
        let result = if transaction.t_to.is_none() {
            //デバック出力
            tracing::info!(
            sender_address =  format_args!("0x{}", hex::encode(sender_address.0)),
            data = %hex::encode(&transaction.data),
            gas = %gas,
            gas_price = %transaction.t_price,
            send_eth = %transaction.t_value,
            "Transaction [CREATE]"
            );
            self.contract_creation(
                state,
                &mut substate,
                sender_address.clone(),
                sender_address.clone(),
                gas,
                transaction.t_price,
                transaction.t_value,
                transaction.data,
                0,
                None,
                true,
                block_header,
            )
        } else {
            let to_address = transaction.t_to.unwrap();
            //デバック出力
            tracing::info!(
            sender_address =  format_args!("0x{}", hex::encode(sender_address.0)),
            to_address =  format_args!("0x{}", hex::encode(to_address.0)),
            data = %hex::encode(&transaction.data),
            gas = %gas,
            gas_price = %transaction.t_price,
            send_eth = %transaction.t_value,
            "Transaction [CALL]"
            );
            self.message_call(
                state,
                &mut substate,
                sender_address.clone(),
                sender_address.clone(),
                to_address.clone(),
                to_address.clone(),
                gas,
                transaction.t_price,
                transaction.t_value,
                transaction.t_value,
                transaction.data,
                0,
                true,
                block_header,
            )
        };

        //払い戻しガス
        match result {
            Ok((gas, _, _)) => {
                let used_gas = transaction.t_gas_limit.saturating_sub(gas);
                let max_refund = if self.version < VersionId::London {
                    //返金の上限がフォークで異なる
                    used_gas / U256::from(2)
                } else {
                    used_gas / U256::from(5)
                };
                let reimburse_u256 = U256::from(substate.a_reimburse.max(0) as u64);
                let reimburse = std::cmp::min(max_refund, reimburse_u256);
                let return_gas = gas.saturating_add(reimburse);
                //送信者への返金
                let reimburse = return_gas.saturating_mul(transaction.t_price);
                if state.is_empty(&sender_address) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&sender_address) {
                        state.add_account(&sender_address, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&sender_address, reimburse);
                //マイナーへの支払い
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(return_gas);
                let f = if self.version < VersionId::London {
                    transaction.t_price
                } else {
                    transaction.t_price - block_header.h_basefee
                };
                let reward = final_billed_gas.saturating_mul(f);
                if state.is_empty(&block_header.h_beneficiary) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&block_header.h_beneficiary) {
                        state.add_account(&block_header.h_beneficiary, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&block_header.h_beneficiary, reward);
                //デバック用
                tracing::info!(
                    beneficiary =  format_args!("0x{}", hex::encode(block_header.h_beneficiary.0)),
                    reward = %reward,
                    reimburse = %reimburse,
                    final_billed_gas = %final_billed_gas,
                    "[マイナーへの支払い]",
                );
                //substate.a_desの処理
                while let Some(address) = substate.a_des.pop() {
                    let address_hash = keccak256(address);
                    state.eth_trie.remove(address_hash.as_slice());
                    state.cache.remove(&address);
                }
                //MPT更新
                for (address, cache_account) in state.cache.iter() {
                    let mut storage_trie = EthTrie::from(state.data.clone(), cache_account.storage_hash).unwrap();
                    //storageの値を書き込む
                    for (key, value) in cache_account.storage.iter() {
                        let key_byte: [u8;32] = key.to_be_bytes();
                        let key_hash = keccak256(key_byte);
                        if value.is_zero() {
                            storage_trie.remove(key_hash.as_slice());
                        } else {
                            let val_rlp_bytes = alloy_rlp::encode(value);
                            storage_trie.insert(key_hash.as_slice(), val_rlp_bytes.as_slice()).unwrap();
                        }
                    }
                    //新しいstorage_rootを取得
                    let storage_root = storage_trie.root_hash().unwrap();
                    //コードハッシュを取得
                    let code_hash = keccak256(cache_account.code.clone());
                    state.code_storage.entry(code_hash).or_insert(cache_account.code.clone());
                    let mpt_account = MptAccount::new(cache_account.nonce, 
                                                  cache_account.balance,
                                                  storage_root,
                                                  code_hash
                                                  );
                    //MPTに書き込む
                    let address_hash = keccak256(address);
                    let mut mpt_accout_rlp_bytes = Vec::new();
                    mpt_account.encode(&mut mpt_accout_rlp_bytes);
                    state.eth_trie.insert(address_hash.as_slice(), mpt_accout_rlp_bytes.as_slice()).unwrap();
                }
                /*
                //substate.a_desの処理
                while let Some(address) = substate.a_des.pop() {
                    let address_hash = keccak256(address);
                    state.eth_trie.remove(address_hash.as_slice());
                }
                */
                //eth_trieのルートハッシュを取得
                let new_state_root  = state.eth_trie.root_hash().unwrap();
                state.update_eth_trie(new_state_root);

                Ok((final_billed_gas, substate.a_log.clone()))
            }
            Err((gas, _, _)) => {
                //送信者への返金
                let reimburse = gas.saturating_mul(transaction.t_price);
                if state.is_empty(&sender_address) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&sender_address) {
                        state.add_account(&sender_address, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&sender_address, reimburse);
                //マイナーへの支払い
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(gas);
                let f = if self.version < VersionId::London {
                    transaction.t_price
                } else {
                    transaction.t_price - block_header.h_basefee
                };
                let reward = final_billed_gas.saturating_mul(f);
                if state.is_empty(&block_header.h_beneficiary) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&block_header.h_beneficiary) {
                        state.add_account(&block_header.h_beneficiary, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&block_header.h_beneficiary, reward);
                //デバック用
                tracing::info!(
                    beneficiary =  format_args!("0x{}", hex::encode(block_header.h_beneficiary.0)),
                    reward = %reward,
                    reimburse = %reimburse,
                    final_billed_gas = %final_billed_gas,
                    "[Err:マイナーへの支払い]",
                );
                //MPT更新
                for (address, cache_account) in state.cache.iter() {
                    let mut storage_trie = EthTrie::from(state.data.clone(), cache_account.storage_hash).unwrap();
                    //storageの値を書き込む
                    for (key, value) in cache_account.storage.iter() {
                        let key_byte: [u8;32] = key.to_be_bytes();
                        let key_hash = keccak256(key_byte);
                        if value.is_zero() {
                            storage_trie.remove(key_hash.as_slice());
                        } else {
                            let val_rlp_bytes = alloy_rlp::encode(value);
                            storage_trie.insert(key_hash.as_slice(), val_rlp_bytes.as_slice()).unwrap();
                        }
                    }
                    //新しいstorage_rootを取得
                    let storage_root = storage_trie.root_hash().unwrap();
                    //コードハッシュを取得
                    let code_hash = keccak256(cache_account.code.clone());
                    state.code_storage.entry(code_hash).or_insert(cache_account.code.clone());
                    let mpt_account = MptAccount::new(cache_account.nonce, 
                                                  cache_account.balance,
                                                  storage_root,
                                                  code_hash
                                                  );
                    //MPTに書き込む
                    let address_hash = keccak256(address);
                    let mut mpt_accout_rlp_bytes = Vec::new();
                    mpt_account.encode(&mut mpt_accout_rlp_bytes);
                    state.eth_trie.insert(address_hash.as_slice(), mpt_accout_rlp_bytes.as_slice()).unwrap();
                }
                //eth_trieのルートハッシュを取得
                let new_state_root  = state.eth_trie.root_hash().unwrap();
                state.update_eth_trie(new_state_root);
                Err((final_billed_gas, Vec::new()))
            }
        }
    }
}

// leviathan.rs の一番下に追加
#[cfg(test)]
mod state_tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    // alloy_primitives の hex を使用して E0433 を解消
    use alloy_primitives::{U256, hex, Address};

    // 署名生成のためのクレート
    use alloy_rlp::{Encodable, Header};
    use bytes::BytesMut;
    use secp256k1::{Message, Secp256k1, SecretKey};
    use sha3::{Digest, Keccak256};

    use crate::leviathan::structs::{BlockHeader, Transaction, VersionId};
    use crate::leviathan::world_state::{Account, WorldState};
    use crate::my_trait::leviathan_trait::TransactionExecution;
    use crate::test::state_parser::StateTestSuite;

    // --- ヘルパー関数 ---

    // 🌟 追加: JSONの "network" 文字列から VersionId を取得する関数
    fn parse_version(network_str: &str) -> VersionId {
        // ">Frontier" や ">=Frontier" などのプレフィックスを削除して純粋なフォーク名にする
        let clean_str = network_str.trim_start_matches(">=").trim_start_matches('>');
        match clean_str {
            "Frontier" => VersionId::Frontier,
            "Homestead" => VersionId::Homestead,
            "EIP150" | "TangerineWhistle" => VersionId::TangerineWhistle,
            "EIP158" | "SpuriousDragon" => VersionId::SpuriousDragon,
            "Byzantium" => VersionId::Byzantium,
            "Constantinople" => VersionId::Constantinople,
            "Petersburg" | "ConstantinopleFix" => VersionId::Petersburg,
            "Istanbul" => VersionId::Istanbul,
            "Berlin" => VersionId::Berlin,
            "London" => VersionId::London,
            "Merge" | "Paris" => VersionId::Merge,
            "Shanghai" => VersionId::Shanghai,
            "Cancun" => VersionId::Cancun,
            _ => VersionId::Latest, // 未知の場合は最新とする
        }
    }

    fn strip_comments(val: &mut serde_json::Value) {
        match val {
            serde_json::Value::Object(map) => {
                map.retain(|k, _| !k.starts_with("//") && !k.starts_with('_'));
                for v in map.values_mut() {
                    strip_comments(v);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    strip_comments(v);
                }
            }
            _ => {}
        }
    }

    fn parse_u256(s: &str) -> U256 {
        let s = s.trim();
        if s.is_empty() {
            return U256::ZERO;
        }
        if s.starts_with("0x") {
            U256::from_str_radix(&s[2..], 16).unwrap_or(U256::ZERO)
        } else {
            U256::from_str_radix(s, 10).unwrap_or(U256::ZERO)
        }
    }

    fn parse_address(s: &str) -> Address {
        let clean_s = s.trim_start_matches("0x");
        let bytes = hex::decode(clean_s).unwrap_or_default();
        let mut addr = [0u8; 20];
        let copy_len = bytes.len().min(20);
        addr[20 - copy_len..].copy_from_slice(&bytes[..copy_len]);
        Address::new(addr)
    }

    fn parse_code(code_str: &str) -> Vec<u8> {
        let s = code_str.trim();
        if s == "{ [[0]] (ADD 1 1) }" {
            return hex::decode("6001600101600055").unwrap();
        }
        hex::decode(s.trim_start_matches("0x")).unwrap_or_default()
    }

    fn sign_transaction(
        nonce: U256,
        gas_price: U256,
        gas_limit: U256,
        to: Option<Address>,
        value: U256,
        data: &[u8],
        secret_key_hex: &str,
    ) -> (U256, U256, U256) {
        // 1. 各要素のRLPペイロード長を事前計算する
        let mut payload_length = 0;
        payload_length += nonce.length();
        payload_length += gas_price.length();
        payload_length += gas_limit.length();

        let to_slice = match &to {
            Some(addr) => addr.0.as_slice(),
            None => &[], // 空のバイト列
        };
        payload_length += to_slice.length();
        payload_length += value.length();
        payload_length += data.length();

        // 2. 必要なメモリを一括で確保し、リストのヘッダーを書き込む
        let mut out = BytesMut::with_capacity(payload_length + 10);
        Header {
            list: true,
            payload_length,
        }
        .encode(&mut out);

        // 3. データを順次エンコード
        // u256_to_minimal_bytes を使わなくても、U256型が勝手にゼロ省略してくれます！
        nonce.encode(&mut out);
        gas_price.encode(&mut out);
        gas_limit.encode(&mut out);
        to_slice.encode(&mut out);
        value.encode(&mut out);
        data.encode(&mut out);

        // RLPエンコードされたバイト列を取り出す
        let rlp_encoded = out.freeze();

        // 4. Keccak256でハッシュ化して32バイトのハッシュを得る
        let mut hasher = Keccak256::new();
        hasher.update(&rlp_encoded);
        let hash: [u8; 32] = hasher.finalize().into();

        // --- 以下、secp256k1 による署名ロジックは既存のまま変更なし ---
        let secp = Secp256k1::new();
        let secret_key_bytes = hex::decode(secret_key_hex).expect("Invalid secret key hex");
        let secret_key = SecretKey::from_slice(&secret_key_bytes).expect("Invalid secret key");
        let message = Message::from_digest_slice(&hash).expect("Invalid message hash");

        let sig = secp.sign_ecdsa_recoverable(message, &secret_key);
        let (recovery_id, sig_bytes) = sig.serialize_compact();

        let r = U256::from_be_slice(&sig_bytes[0..32]);
        let s = U256::from_be_slice(&sig_bytes[32..64]);

        let rec_id_i32 = i32::from(recovery_id);
        let v = U256::from(rec_id_i32 as u64 + 27);

        (v, r, s)
    }

    #[test]
    fn state_test() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        // 対象のディレクトリ
        let test_dir = "MPTTest/stCreate2";

        let paths = std::fs::read_dir(test_dir)
            .unwrap_or_else(|_| panic!("Failed to read test directory: {}", test_dir));

        let mut total_files = 0;
        let mut pass_cases_count = 0;
        let mut total_cases_count = 0;

        for path in paths {
            let path = path.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            total_files += 1;
            let file_name = path.file_name().unwrap().to_str().unwrap();
            println!("\n==================================================");
            println!(" Loading File: {}", file_name);

            let json_data = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("Failed to read JSON file: {}", file_name));

            let mut raw_json: serde_json::Value = serde_json::from_str(&json_data).unwrap();
            strip_comments(&mut raw_json);

            let suite: StateTestSuite = serde_json::from_value(raw_json)
                .unwrap_or_else(|_| panic!("Failed to parse JSON in {}", file_name));

            for (test_name, test_data) in suite.tests {
                println!("--------------------------------------------------");
                println!("▶ Running State Test: {}", test_name);

                // 🌟 改修ポイント：postの中のネットワーク(フォーク)ごとにループを回す
                for (network_str, post_states) in &test_data.post {
                    let version = parse_version(network_str);

                    // 現在サポートしていない古いフォークや未来のフォークをスキップしたい場合はここで弾く
                    // if version < VersionId::Istanbul { continue; }

                    for (post_idx, post_state) in post_states.iter().enumerate() {
                        total_cases_count += 1;

                        let data_idx = post_state.indexes.data.first() as usize;
                        let gas_idx = post_state.indexes.gas.first() as usize;
                        let value_idx = post_state.indexes.value.first() as usize;

                        let tx_data_str = &test_data.transaction.data[data_idx];
                        let gas_limit_str = &test_data.transaction.gas_limit[gas_idx];
                        let value_str = &test_data.transaction.value[value_idx];

                        println!("  [Matrix {}] Network: {}", post_idx, network_str);
                        // 1. WorldStateの初期化 (必ず毎ループ初期化する！)
                        let mut state = WorldState::new();

                        for (addr_str, acc_data) in &test_data.pre {
                            let addr = parse_address(addr_str);

                            // 🌟 【追加】アカウントごとに初期の Storage Trie を構築する！
                            let mut storage_trie = EthTrie::new(state.data.clone());
                            let mut storage = HashMap::new();

                            if let Some(st) = &acc_data.storage {
                                for (k, v) in st {
                                    let key_u256 = parse_u256(k);
                                    let val_u256 = parse_u256(v);
                                    storage.insert(key_u256, val_u256);

                                    // MPTに初期ストレージの値をインサート
                                    let key_byte: [u8; 32] = key_u256.to_be_bytes();
                                    let key_hash = keccak256(key_byte);
                                    let val_rlp = alloy_rlp::encode(val_u256);
                                    storage_trie.insert(key_hash.as_slice(), val_rlp.as_slice()).unwrap();
                                }
                            }
                            // 初期ストレージの正しいルートハッシュを確定させる！
                            let initial_storage_root = storage_trie.root_hash().unwrap();

                            let nonce = acc_data.nonce.as_ref().map(|n| parse_u256(n).try_into().unwrap_or(0)).unwrap_or(0);
                            let balance = acc_data.balance.as_ref().map(|b| parse_u256(b)).unwrap_or(U256::ZERO);
                            let code = acc_data.code.as_ref().map(|c| parse_code(c)).unwrap_or_default();

                            let code_hash = keccak256(&code);
                            state.code_storage.insert(code_hash, code.clone());

                            let account = Account {
                                nonce,
                                balance,
                                storage,
                                code,
                                storage_hash: initial_storage_root, // 🌟 ダミーではなく本物をセット！
                            };
                            state.add_account(&addr, account);

                            // 🌟 【追加】さらに全体デスク (State Trie) にもアカウントを登録しておく！
                            let mpt_account = MptAccount::new(nonce, balance, initial_storage_root, code_hash);
                            let addr_hash = keccak256(&addr);
                            let mut mpt_rlp = Vec::new();
                            mpt_account.encode(&mut mpt_rlp);
                            state.eth_trie.insert(addr_hash.as_slice(), mpt_rlp.as_slice()).unwrap();
                        }

                        // 🌟 【追加】トランザクション実行「前」の State Root を確認してみる！
                        let pre_state_root = state.eth_trie.root_hash().unwrap();
                        println!("    [Pre-State] Initial State Root: {}", pre_state_root);

                        // --- ここから下が Env情報の構築 と トランザクション実行 (leviathan.execution) ---

                        // Env情報の構築
                        let block_header = BlockHeader {
                            h_beneficiary: parse_address(&test_data.env.current_coinbase),
                            h_timestamp: parse_u256(&test_data.env.current_timestamp),
                            h_number: parse_u256(&test_data.env.current_number),
                            h_prevrandao: parse_u256(&test_data.env.current_difficulty),
                            h_gaslimit: parse_u256(&test_data.env.current_gas_limit),
                            h_basefee: U256::ZERO,
                        };

                        // トランザクション情報の構築
                        let tx_data = parse_code(tx_data_str);
                        let gas_limit = parse_u256(gas_limit_str);
                        let value = parse_u256(value_str);
                        let to_address = if test_data.transaction.to.is_empty() {
                            None
                        } else {
                            Some(parse_address(&test_data.transaction.to))
                        };
                        let nonce = parse_u256(&test_data.transaction.nonce);
                        let gas_price = parse_u256(&test_data.transaction.gas_price);
                        let secret_key_hex = test_data.transaction.secret_key.trim_start_matches("0x");

                        let (v, r, s) = sign_transaction(
                            nonce, gas_price, gas_limit, to_address.clone(), value, &tx_data, secret_key_hex,
                        );

                        let transaction = Transaction {
                            data: tx_data,
                            t_to: to_address,
                            t_gas_limit: gas_limit,
                            t_price: gas_price,
                            t_value: value,
                            t_nonce: nonce.try_into().unwrap_or(0),
                            t_w: v, t_r: r, t_s: s,
                        };

                        // 2. 実行
                        let mut leviathan = LEVIATHAN::new(version);
                        let _result = leviathan.execution(&mut state, transaction, &block_header);

                        // 3. 🌟 究極の検証フェーズ：State Root Hashの比較
                        // post_state.hash の文字列を B256 型にパース
                        let expected_hash: alloy_primitives::B256 = post_state.hash.parse()
                            .expect("Failed to parse expected hash");

                        // 実装した EthTrie から最終的なルートハッシュを取得
                        let actual_hash = state.eth_trie.root_hash().unwrap();

                        if actual_hash == expected_hash {
                            println!("    => Success! State Root Matches: {}", expected_hash);
                            pass_cases_count += 1;
                        } else {
                            println!("    => FAILED!");
                            println!("       Expected: {}", expected_hash);
                            println!("       Actual  : {}", actual_hash);
                            println!("\n=== 🔍 最終ステートのダンプ (Cache内の最新状態) ===");
                            for (address, account) in &state.cache {
                                println!("Address: 0x{}", alloy_primitives::hex::encode(address.0));
                                println!("  Nonce       : {}", account.nonce);
                                println!("  Balance     : {}", account.balance);
                                println!("  Code (len)  : {} bytes", account.code.len());
                                println!("  Storage:");
                                if account.storage.is_empty() {
                                    println!("    (empty)");
                                } else {
                                    // 見やすいようにキーでソートして出力
                                    let mut keys: Vec<_> = account.storage.keys().collect();
                                    keys.sort();
                                    for k in keys {
                                        let v = account.storage.get(k).unwrap();
                                        println!("    [{}] -> {}", k, v);
                                    }
                                }
                                println!("  StorageRoot : {}", account.storage_hash);
                                println!("---------------------------------------------------");
                            }
                            println!("===================================================\n");
                            assert_eq!(actual_hash, expected_hash, "State root mismatch in test: {}", test_name);
                        }
                    }
                }
            }
        }
        println!("\n==================================================");
        println!("最終結果: {} ファイル中、{} / {} のテストケースをクリアしました！", total_files, pass_cases_count, total_cases_count);
        println!("==================================================\n");
    }
}
