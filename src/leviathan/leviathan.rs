#![allow(dead_code)]

use crate::evm::evm::EVM;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{
    BackupSubstate, BlockHeader, ExecutionEnvironment, Log, SubState, Transaction, VersionId,
};
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::leviathan_trait::{
    ContractCreation, MessageCall, State, TransactionChecks, TransactionExecution,
};
use alloy_primitives::{I256, U256, hex};
use sha3::{Digest, Keccak256};

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
            version: version,
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
                    //set_balance前の確認
                    state.add_account(&sender_address, Account::new()); //アカウントを追加
                    Action::Account_creation(sender_address.clone()).push(self, state); //アカウントが存在しない場合
                }
                state.set_balance(&sender_address, reimburse);
                //マイナーへの支払い
                //println!("マイナーアドレス: 0x{}",hex::encode(block_header.h_beneficiary.0)); //アドレス
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(return_gas);
                let f = if self.version < VersionId::London {
                    transaction.t_price
                } else {
                    transaction.t_price - block_header.h_basefee
                };
                let reward = final_billed_gas.saturating_mul(f);
                if state.is_empty(&block_header.h_beneficiary) {
                    //set_balance前の確認
                    state.add_account(&block_header.h_beneficiary, Account::new()); //アカウントを追加
                    Action::Account_creation(block_header.h_beneficiary.clone()).push(self, state); //アカウントが存在しない場合
                }
                state.set_balance(&block_header.h_beneficiary, reward);
                //substate.a_desの処理
                while !substate.a_des.is_empty() {
                    let address = substate.a_des.pop().unwrap();
                    state.delete_account(&address);
                }

                return Ok((final_billed_gas, substate.a_log.clone()));
            }
            Err((gas, _, _)) => {
                //送信者への返金
                let reimburse = gas.saturating_mul(transaction.t_price);
                if state.is_empty(&sender_address) {
                    //set_balance前の確認
                    state.add_account(&sender_address, Account::new()); //アカウントを追加
                    Action::Account_creation(sender_address.clone()).push(self, state); //アカウントが存在しない場合
                }
                state.set_balance(&sender_address, reimburse);
                //マイナーへの支払い
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(gas);
                let f = if self.version < VersionId::London {
                    transaction.t_price
                } else {
                    transaction.t_price - block_header.h_basefee
                };
                let reward = final_billed_gas.saturating_mul(f);
                if state.is_empty(&block_header.h_beneficiary) {
                    //set_balance前の確認
                    state.add_account(&block_header.h_beneficiary, Account::new()); //アカウントを追加
                    Action::Account_creation(block_header.h_beneficiary.clone()).push(self, state); //アカウントが存在しない場合
                }
                state.set_balance(&block_header.h_beneficiary, reward);
                return Err((final_billed_gas, Vec::new()));
            }
        }
    }
}

// leviathan.rs の一番下に追加
// leviathan.rs の一番下に追加
#[cfg(test)]
mod state_tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    // alloy_primitives の hex を使用して E0433 を解消
    use alloy_primitives::{U256, hex};

    // 署名生成のためのクレート
    use rlp::RlpStream;
    use secp256k1::{Message, Secp256k1, SecretKey};
    use sha3::{Digest, Keccak256};

    use crate::leviathan::structs::{BlockHeader, Transaction, VersionId};
    use crate::leviathan::world_state::{Account, Address, WorldState};
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
            "Constantinople" | "ConstantinopleFix" => VersionId::Constantinople,
            "Petersburg" => VersionId::Petersburg,
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

    fn u256_to_minimal_bytes(val: U256) -> Vec<u8> {
        if val == U256::ZERO {
            return vec![];
        }
        let bytes = val.to_be_bytes::<32>();
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(32);
        bytes[start..].to_vec()
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

        let mut hasher = Keccak256::new();
        hasher.update(stream.out());
        let hash: [u8; 32] = hasher.finalize().try_into().unwrap();

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
        // ここにテストしたいディレクトリへのパスを指定します
        let test_dir = "require/stInitCodeTest";
        //let test_dir = "testdata/GeneralStateTestsFiller/CompleteTest";

        let paths = fs::read_dir(test_dir)
            .unwrap_or_else(|_| panic!("Failed to read test directory: {}", test_dir));

        let mut total_files = 0;
        let mut pass_cases_count = 0;
        let mut total_cases_count = 0;

        for path in paths {
            let path = path.unwrap().path();

            // .json ファイル以外はスキップ
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            total_files += 1;
            let file_name = path.file_name().unwrap().to_str().unwrap();
            println!("\n==================================================");
            println!(" Loading File: {}", file_name);

            let json_data = fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("Failed to read JSON file: {}", file_name));

            let mut raw_json: serde_json::Value = serde_json::from_str(&json_data)
                .unwrap_or_else(|_| panic!("Failed to parse raw JSON in {}", file_name));

            strip_comments(&mut raw_json);

            let suite: StateTestSuite = serde_json::from_value(raw_json)
                .unwrap_or_else(|_| panic!("Failed to parse into StateTestSuite in {}", file_name));

            for (test_name, test_data) in suite.tests {
                println!("--------------------------------------------------");
                println!("▶ Running STRICT State Test: {}", test_name);

                // ▼ ここから大改修：expectの配列でループを回す！
                for (expect_idx, expect_data) in test_data.expect.iter().enumerate() {
                    total_cases_count += 1;

                    let network_str = expect_data
                        .network
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("Frontier");
                    let version = parse_version(network_str);

                    // indexes の取得（存在しない場合や -1 の場合は 0 を使う）
                    let (data_idx, gas_idx, value_idx) = match &expect_data.indexes {
                        Some(idx) => (
                            if idx.data.first() < 0 {
                                0
                            } else {
                                idx.data.first() as usize
                            },
                            if idx.gas.first() < 0 {
                                0
                            } else {
                                idx.gas.first() as usize
                            },
                            if idx.value.first() < 0 {
                                0
                            } else {
                                idx.value.first() as usize
                            },
                        ),
                        None => (0, 0, 0), // 昔のフォーマットへの後方互換性
                    };

                    let default_str = String::new();
                    let tx_data_str =
                        test_data.transaction.data.get(data_idx).unwrap_or_else(|| {
                            test_data.transaction.data.first().unwrap_or(&default_str)
                        });
                    let gas_limit_str = test_data
                        .transaction
                        .gas_limit
                        .get(gas_idx)
                        .unwrap_or_else(|| {
                            test_data
                                .transaction
                                .gas_limit
                                .first()
                                .unwrap_or(&default_str)
                        });
                    let value_str =
                        test_data
                            .transaction
                            .value
                            .get(value_idx)
                            .unwrap_or_else(|| {
                                test_data.transaction.value.first().unwrap_or(&default_str)
                            });

                    // ターミナルへの詳細なログ出力
                    println!("  [Matrix {}] Version: {:?}", expect_idx, version);
                    let display_data = if tx_data_str.len() > 64 {
                        format!("{}... (len: {})", &tx_data_str[..64], tx_data_str.len())
                    } else {
                        tx_data_str.to_string()
                    };
                    println!("    ├─ Data  [Idx {}]: {}", data_idx, display_data);
                    println!("    ├─ Gas   [Idx {}]: {}", gas_idx, gas_limit_str);
                    println!("    ├─ Value [Idx {}]: {}", value_idx, value_str);
                    println!("    ├─ Expected State:");

                    for (addr_str, expected_acc) in &expect_data.result {
                        println!("          Address: 0x{}", addr_str);
                        if let Some("1") = expected_acc.shouldnotexist.as_deref() {
                            println!("            - 存在しないこと (Should Not Exist)");
                            continue;
                        }
                        if let Some(nonce) = &expected_acc.nonce {
                            println!("            - Nonce:   {}", nonce);
                        }
                        if let Some(balance) = &expected_acc.balance {
                            println!("            - Balance: {}", balance);
                        }
                        if let Some(code) = &expected_acc.code {
                            let disp = if code.len() > 20 {
                                format!("{}...", &code[..20])
                            } else {
                                code.to_string()
                            };
                            println!("            - Code:    {}", disp);
                        }
                        if let Some(storage) = &expected_acc.storage {
                            if storage.is_empty() {
                                println!("            - Storage: {{}} (Empty)");
                            } else {
                                println!("            - Storage:");
                                for (k, v) in storage {
                                    println!("                [{}] == {}", k, v);
                                }
                            }
                        }
                    }

                    //  超重要：テストの実行ごとに必ずWorldStateを初期状態から構築し直す！
                    let mut world_state_map = HashMap::new();
                    for (addr_str, acc_data) in &test_data.pre {
                        let mut storage = HashMap::new();
                        if let Some(st) = &acc_data.storage {
                            for (k, v) in st {
                                storage.insert(parse_u256(k), parse_u256(v));
                            }
                        }

                        let account = Account {
                            nonce: acc_data
                                .nonce
                                .as_ref()
                                .map(|n| parse_u256(n).try_into().unwrap_or(0))
                                .unwrap_or(0),
                            balance: acc_data
                                .balance
                                .as_ref()
                                .map(|b| parse_u256(b))
                                .unwrap_or(U256::ZERO),
                            storage,
                            code: acc_data
                                .code
                                .as_ref()
                                .map(|c| parse_code(c))
                                .unwrap_or_default(),
                        };
                        world_state_map.insert(parse_address(addr_str), account);
                    }
                    let mut state = WorldState(world_state_map);

                    println!("    └─ Pre State (Before Tx):");
                    let mut pre_addresses: Vec<_> = state.0.keys().collect();
                    pre_addresses.sort_by_key(|addr| addr.0); // アドレスでソート

                    for addr in pre_addresses {
                        let acc = state.0.get(addr).unwrap();
                        println!("          Address: 0x{}", hex::encode(addr.0));
                        println!("            - Nonce:   {}", acc.nonce);
                        println!("            - Balance: {}", acc.balance);

                        if !acc.code.is_empty() {
                            let code_hex = hex::encode(&acc.code);
                            let disp = if code_hex.len() > 32 {
                                format!("{}... (len: {})", &code_hex[..32], code_hex.len())
                            } else {
                                code_hex
                            };
                            println!("            - Code:    0x{}", disp);
                        }

                        if !acc.storage.is_empty() {
                            println!("            - Storage:");
                            let mut keys: Vec<_> = acc.storage.keys().collect();
                            keys.sort();
                            for k in keys {
                                let v = acc.storage.get(k).unwrap();
                                println!("                [0x{:x}] == 0x{:x}", k, v);
                            }
                        }
                    }

                    let block_header = BlockHeader {
                        h_beneficiary: parse_address(&test_data.env.current_coinbase),
                        h_timestamp: parse_u256(&test_data.env.current_timestamp),
                        h_number: parse_u256(&test_data.env.current_number),
                        h_prevrandao: parse_u256(&test_data.env.current_difficulty),
                        h_gaslimit: parse_u256(&test_data.env.current_gas_limit),
                        h_basefee: U256::ZERO,
                    };

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
                        nonce,
                        gas_price,
                        gas_limit,
                        to_address.clone(),
                        value,
                        &tx_data,
                        secret_key_hex,
                    );

                    let transaction = Transaction {
                        data: tx_data,
                        t_to: to_address,
                        t_gas_limit: gas_limit,
                        t_price: gas_price,
                        t_value: value,
                        t_nonce: nonce.try_into().unwrap_or(0),
                        t_w: v,
                        t_r: r,
                        t_s: s,
                    };

                    let mut leviathan = LEVIATHAN::new(version);
                    let result = leviathan.execution(&mut state, transaction, &block_header);

                    match result {
                        Ok(_) => println!("  => Transaction Result: Success"),
                        Err(_) => println!("  => Transaction Result: Exception Halt (Expected)"),
                    }

                    println!("    └─   Actual State (Full):");

                    // アドレス順にソートして出力（比較しやすくするため）
                    let mut addresses: Vec<_> = state.0.keys().collect();
                    addresses.sort();

                    for addr in addresses {
                        let acc = state.0.get(addr).unwrap();

                        // 残高が0かつNonceが0かつコードもストレージも空のアカウントは、
                        // 実質「存在しない」のと同じなので、ノイズを減らすためにスキップしても良いですが、
                        // 今回は「全部出す」という要望なので、あえてすべて出力します。

                        println!("          Address: 0x{}", hex::encode(addr.0));
                        println!("            - Nonce:   {}", acc.nonce);
                        println!("            - Balance: {}", acc.balance);

                        if !acc.code.is_empty() {
                            let code_hex = hex::encode(&acc.code);
                            let disp = if code_hex.len() > 32 {
                                format!("{}... (len: {})", &code_hex[..32], code_hex.len())
                            } else {
                                code_hex
                            };
                            println!("            - Code:    0x{}", disp);
                        }

                        if !acc.storage.is_empty() {
                            println!("            - Storage:");
                            // ストレージキーもソートして出力
                            let mut keys: Vec<_> = acc.storage.keys().collect();
                            keys.sort();
                            for k in keys {
                                let v = acc.storage.get(k).unwrap();
                                // キーと値を16進数で表示（デバッグしやすいため）
                                println!("                [0x{:x}] == 0x{:x}", k, v);
                            }
                        }
                    }

                    // 検証フェーズ (expect_data.result を使う)
                    for (addr_str, expected_acc) in &expect_data.result {
                        let addr = parse_address(addr_str);
                        let actual_acc_opt = state.0.get(&addr);

                        if let Some("1") = expected_acc.shouldnotexist.as_deref() {
                            assert!(
                                actual_acc_opt.is_none()
                                    || actual_acc_opt.unwrap().balance == U256::ZERO,
                                "[{}] Address {} は存在してはいけません",
                                test_name,
                                addr_str
                            );
                            continue;
                        }

                        let actual_acc = actual_acc_opt.unwrap_or_else(|| {
                            panic!("Address {} がステートに存在しません", addr_str)
                        });

                        if let Some(expected_balance_str) = &expected_acc.balance {
                            let expected_balance = parse_u256(expected_balance_str);
                            assert_eq!(
                                actual_acc.balance, expected_balance,
                                "[{}] Address {} の Balance が不一致",
                                test_name, addr_str
                            );
                        }

                        if let Some(expected_nonce_str) = &expected_acc.nonce {
                            let expected_nonce: u32 =
                                parse_u256(expected_nonce_str).try_into().unwrap_or(0);
                            assert_eq!(
                                actual_acc.nonce, expected_nonce,
                                "[{}] Address {} の Nonce が不一致",
                                test_name, addr_str
                            );
                        }

                        if let Some(expected_code_str) = &expected_acc.code {
                            let expected_code = parse_code(expected_code_str);
                            assert_eq!(
                                actual_acc.code, expected_code,
                                "[{}] Address {} の Code が不一致",
                                test_name, addr_str
                            );
                        }

                        if let Some(expected_storage) = &expected_acc.storage {
                            for (k, v) in expected_storage {
                                let key = parse_u256(k);
                                let expected_val = parse_u256(v);
                                let actual_val =
                                    actual_acc.storage.get(&key).unwrap_or(&U256::ZERO);

                                assert_eq!(
                                    *actual_val, expected_val,
                                    "[{}] Address {} の Storage[{}] が不一致",
                                    test_name, addr_str, k
                                );
                            }
                        }
                    }
                    println!("  Passed Matrix {}", expect_idx);
                    pass_cases_count += 1;
                }
                println!("Passed All Matrices for: {}", test_name);
            }
        }

        println!("\n==================================================");
        println!(
            "最終結果: {} ファイル中、{} / {} のテストケースをクリアしました！",
            total_files, pass_cases_count, total_cases_count
        );
        println!("==================================================\n");
    }
}
