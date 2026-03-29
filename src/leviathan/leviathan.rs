#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution, TransactionChecks, ContractCreation, MessageCall};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction, BlockHeader, BackupSubstate};
use crate::evm::evm::EVM;
use sha3::{Keccak256, Digest};


pub struct LEVIATHAN {
    pub journal: Vec<Action>,
    pub substate_backup: BackupSubstate,
}

impl LEVIATHAN {
    pub fn new() -> Self {
        Self{journal:Vec::<Action>::new(),substate_backup:BackupSubstate::new()}
    }

    pub fn merge(&mut self, mut children: LEVIATHAN) {
        self.journal.append(&mut children.journal);
    }

}

impl TransactionExecution for LEVIATHAN {
     fn execution(&mut self, state: &mut WorldState, transaction:Transaction, block_header: &BlockHeader) -> Result<(U256, Vec<Log>),(U256, Vec<Log>)> {

        //=======ステップ1===========
        //【初期ガスの計算】
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
        //【事前支払いコスト】
         let max_cost = transaction.t_gas_limit.saturating_mul(transaction.t_price) + transaction.t_value;
       //【トランザクションの事前検証】
         let sender_address = LEVIATHAN::transaction_checks(state, &transaction, &all_gas, &max_cost, block_header);
         if sender_address.is_err() {
             return Err((U256::ZERO, Vec::new()));
         }
         let sender_address = sender_address.unwrap();

         //=======ステップ2===========
         //【Nonceの加算】
         state.inc_nonce(&sender_address);
         //【前払いガス代の徴収】
         let gas = state.buy_gas(&sender_address, transaction.t_price, transaction.t_value);
         //ここからロールバックの起点:ロールバックが起きたらこの状態にする
         let mut substate = SubState::new();
    
         //=======ステップ3===========
         let result = if transaction.t_to.is_none() {
             self.contract_creation(state, &mut substate, sender_address.clone(), sender_address.clone(), gas.unwrap(), transaction.t_price, 
                                    transaction.t_value, transaction.data, 0, None, true, block_header)
         }else{
             let to_address = transaction.t_to.unwrap();
             self.message_call(state, &mut substate, sender_address.clone(), sender_address.clone(), to_address.clone(), to_address.clone(),
                    gas.unwrap(), transaction.t_price, transaction.t_value, transaction.t_value, transaction.data, 0, true, block_header) 

         };
        
         //払い戻しガス
         match result {
             Ok((gas, _)) => {
                 let used_gas = transaction.t_gas_limit.saturating_sub(gas);
                 let max_refund = used_gas / U256::from(5);
                 let reimburse_u256 = U256::from(substate.a_reimburse.max(0) as u64);
                 let reimburse = std::cmp::min(max_refund, reimburse_u256);
                 let return_gas = gas.saturating_add(reimburse);
                //送信者への返金
                let reimburse = return_gas.saturating_mul(transaction.t_price);
                state.set_balance(&sender_address, reimburse);
                //マイナーへの支払い
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(return_gas);
                let f = transaction.t_price - block_header.h_basefee;
                let reward = final_billed_gas.saturating_mul(f);
                state.set_balance(&block_header.h_beneficiary, reward);
                return Ok((final_billed_gas, substate.a_log.clone()));
             }
             Err((gas, _)) => {
                //送信者への返金
                let reimburse = gas.saturating_mul(transaction.t_price);
                state.set_balance(&sender_address, reimburse);
                //マイナーへの支払い
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(gas);
                let f = transaction.t_price - block_header.h_basefee;
                let reward = final_billed_gas.saturating_mul(f);
                state.set_balance(&block_header.h_beneficiary, reward);
                return Err((final_billed_gas, Vec::new()));
             },
         }
         
     }
}



// leviathan.rs の一番下に追加
#[cfg(test)]
mod state_tests {
    use super::*; 
    use std::fs;
    use std::collections::HashMap;
    
    // alloy_primitives の hex を使用して E0433 を解消
    use alloy_primitives::{U256, hex};

    // 署名生成のためのクレート
    use secp256k1::{Secp256k1, SecretKey, Message};
    use sha3::{Keccak256, Digest};
    use rlp::RlpStream;

    use crate::test::state_parser::StateTestSuite;
    use crate::leviathan::world_state::{WorldState, Account, Address};
    use crate::leviathan::structs::{BlockHeader, Transaction};
    use crate::my_trait::leviathan_trait::TransactionExecution;

    // --- ヘルパー関数 ---
    // コメントキー("//" や "_")を再帰的に削除するヘルパー関数
    fn strip_comments(val: &mut serde_json::Value) {
        match val {
            serde_json::Value::Object(map) => {
                // "//" または "_" で始まるキーを削除
                map.retain(|k, _| !k.starts_with("//") && !k.starts_with('_'));
                // 残った値の中も再帰的にチェック
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
        if s.is_empty() { return U256::ZERO; }
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

    // U256をRLP用に先頭ゼロを取り除いた最小バイト列に変換する関数
    fn u256_to_minimal_bytes(val: U256) -> Vec<u8> {
        if val == U256::ZERO {
            return vec![];
        }
        let bytes = val.to_be_bytes::<32>();
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(32);
        bytes[start..].to_vec()
    }

    // 秘密鍵から署名 (v, r, s) を生成する厳密な関数
fn sign_transaction(
        nonce: U256,
        gas_price: U256,
        gas_limit: U256,
        to: Option<Address>,
        value: U256,
        data: &[u8],
        secret_key_hex: &str,
    ) -> (U256, U256, U256) {
        // 1. FrontierレガシートランザクションのRLPエンコード: [nonce, gasPrice, gasLimit, to, value, data]
        let mut stream = RlpStream::new_list(6);
        stream.append(&u256_to_minimal_bytes(nonce));
        stream.append(&u256_to_minimal_bytes(gas_price));
        stream.append(&u256_to_minimal_bytes(gas_limit));
        
        if let Some(addr) = to {
            stream.append(&addr.0.as_ref());
        } else {
            stream.append_empty_data();
        }
        
        stream.append(&u256_to_minimal_bytes(value));
        stream.append(&data);

        // 2. Keccak256でハッシュ化
        let mut hasher = Keccak256::new();
        hasher.update(stream.out());
        let hash: [u8; 32] = hasher.finalize().try_into().unwrap();

        // 3. 秘密鍵をパースしてECDSA署名 (Recoverable)
        let secp = Secp256k1::new();
        let secret_key_bytes = hex::decode(secret_key_hex).expect("Invalid secret key hex");
        let secret_key = SecretKey::from_slice(&secret_key_bytes).expect("Invalid secret key");
        let message = Message::from_digest_slice(&hash).expect("Invalid message hash");
        
        // 【修正 E0277】 &message ではなく message を値として渡す
        let sig = secp.sign_ecdsa_recoverable(message, &secret_key);
        let (recovery_id, sig_bytes) = sig.serialize_compact();
        
        // 4. v, r, s の抽出
        let r = U256::from_be_slice(&sig_bytes[0..32]);
        let s = U256::from_be_slice(&sig_bytes[32..64]);
        
        // 【修正 E0599】 to_i32() の代わりに i32::from() または .to_byte() を使用
        let rec_id_i32 = i32::from(recovery_id); 
        let v = U256::from(rec_id_i32 as u64 + 27);

        (v, r, s)
    }

    #[test]
    fn test_add11_state_strict() {
        let test_file = "testdata/GeneralStateTestsFiller/stExample/add11Filler.json";
        let json_data = fs::read_to_string(test_file).expect("Failed to read JSON file");
        
        // 1. 一旦型なしの柔軟なValueとして読み込む
        let mut raw_json: serde_json::Value = serde_json::from_str(&json_data).expect("Failed to parse raw JSON");
        
        // 2. コメント類 ("//comment", "_info" など) をすべて取り除く
        strip_comments(&mut raw_json);
        
        // 3. 綺麗になったJSONデータを、本来の StateTestSuite 構造体にパースする
        let suite: StateTestSuite = serde_json::from_value(raw_json).expect("Failed to parse into StateTestSuite");
        

        for (test_name, test_data) in suite.tests {
            println!("========================================");
            println!("▶ Running STRICT State Test: {}", test_name);

            // 1. Env (BlockHeader) 構築...
            let block_header = BlockHeader {
                h_beneficiary: parse_address(&test_data.env.current_coinbase),
                h_timestamp: parse_u256(&test_data.env.current_timestamp),
                h_number: parse_u256(&test_data.env.current_number),
                h_prevrandao: parse_u256(&test_data.env.current_difficulty),
                h_gaslimit: parse_u256(&test_data.env.current_gas_limit),
                h_basefee: U256::ZERO,
            };

            // 2. Pre State 構築...
            let mut world_state_map = HashMap::new();
            for (addr_str, acc_data) in &test_data.pre {
                let mut storage = HashMap::new();
                if let Some(st) = &acc_data.storage {
                    for (k, v) in st {
                        storage.insert(parse_u256(k), parse_u256(v));
                    }
                }
                
                let account = Account {
                    nonce: acc_data.nonce.as_ref().map(|n| parse_u256(n).try_into().unwrap_or(0)).unwrap_or(0),
                    balance: acc_data.balance.as_ref().map(|b| parse_u256(b)).unwrap_or(U256::ZERO),
                    storage,
                    code: acc_data.code.as_ref().map(|c| parse_code(c)).unwrap_or_default(),
                };
                world_state_map.insert(parse_address(addr_str), account);
            }
            let mut state = WorldState(world_state_map);

            // 3. トランザクション・パラメータ取得
            let tx_data = parse_code(&test_data.transaction.data[0]);
            let to_address = if test_data.transaction.to.is_empty() {
                None
            } else {
                Some(parse_address(&test_data.transaction.to))
            };
            
            let nonce = parse_u256(&test_data.transaction.nonce);
            let gas_price = parse_u256(&test_data.transaction.gas_price);
            let gas_limit = parse_u256(&test_data.transaction.gas_limit[0]);
            let value = parse_u256(&test_data.transaction.value[0]);
            
            let secret_key_hex = test_data.transaction.secret_key.trim_start_matches("0x");
            
            // 4. アプローチA: 署名生成！
            let (v, r, s) = sign_transaction(
                nonce, gas_price, gas_limit, to_address.clone(), value, &tx_data, secret_key_hex
            );

            let transaction = Transaction {
                data: tx_data,
                t_to: to_address,
                t_gas_limit: gas_limit,
                t_price: gas_price,
                t_value: value,
                // 【修正 E0308】 Transaction構造体の型に合わせて usize または u64 にキャスト
                t_nonce: nonce.try_into().unwrap_or(0),
                // 【修正 E0560】 t_v を t_w に修正
                t_w: v,
                t_r: r,
                t_s: s,
            };

            // 5. トランザクション実行
            let mut leviathan = LEVIATHAN::new();
            let result = leviathan.execution(&mut state, transaction, &block_header);

            assert!(result.is_ok(), "Transaction execution failed: {:?}", result.err());
            // 6. Post Condition (Expect) の検証
            let expect_data = &test_data.expect[0];
            for (addr_str, expected_acc) in &expect_data.result {
                let addr = parse_address(addr_str);
                let actual_acc_opt = state.0.get(&addr);

                if let Some("1") = expected_acc.shouldnotexist.as_deref() {
                    assert!(
                        actual_acc_opt.is_none() || actual_acc_opt.unwrap().balance == U256::ZERO,
                        "[{}] Address {} は存在してはいけません", test_name, addr_str
                    );
                    continue;
                }

                let actual_acc = actual_acc_opt.expect(&format!("Address {} がステートに存在しません", addr_str));

                if let Some(expected_balance_str) = &expected_acc.balance {
                    let expected_balance = parse_u256(expected_balance_str);
                    assert_eq!(
                        actual_acc.balance, expected_balance,
                        "[{}] Address {} の Balance が不一致", test_name, addr_str
                    );
                }

                if let Some(expected_nonce_str) = &expected_acc.nonce {
                    // E0308, E0277解消: u64 -> u32
                    let expected_nonce: u32 = parse_u256(expected_nonce_str).try_into().unwrap_or(0);
                    assert_eq!(
                        actual_acc.nonce, expected_nonce,
                        "[{}] Address {} の Nonce が不一致", test_name, addr_str
                    );
                }

                if let Some(expected_code_str) = &expected_acc.code {
                    let expected_code = parse_code(expected_code_str);
                    assert_eq!(
                        actual_acc.code, expected_code,
                        "[{}] Address {} の Code が不一致", test_name, addr_str
                    );
                }

                if let Some(expected_storage) = &expected_acc.storage {
                    for (k, v) in expected_storage {
                        let key = parse_u256(k);
                        let expected_val = parse_u256(v);
                        let actual_val = actual_acc.storage.get(&key).unwrap_or(&U256::ZERO);
                        
                        assert_eq!(
                            *actual_val, expected_val,
                            "[{}] Address {} の Storage[{}] が不一致", test_name, addr_str, k
                        );
                    }
                }
            }
            println!("✅ Passed: {}", test_name);
        }
    }
}
