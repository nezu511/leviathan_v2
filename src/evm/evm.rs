#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::evm_trait::{Xi, Gfunction, Zfunction, Hfunction, Ofunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use crate::leviathan::leviathan::LEVIATHAN;

pub struct EVM {
    pub gas: U256,
    pub pc: usize,
    pub memory: Vec<u8>,
    pub active_words: usize,
    pub stack: Vec<U256>,
    pub return_back: Vec<u8>,
    pub safe_jump: Vec<u8>,
}


impl EVM {
    pub fn new(execution_environment: &ExecutionEnvironment) -> Self {

        //安全なjump先のリストを作成
        let code = execution_environment.i_byte.clone();
        let code_len = code.len();
        let mut safe_jump:Vec<u8> = vec![0;code_len];
        let mut pointer = 0usize;
        while pointer < code_len {
            let x = code[pointer];
            match x {
                0x5b => {
                    safe_jump[pointer] = 1;
                    pointer += 1;
                },
                0x60 ..=0x7f => {
                    let val = x as usize;
                    pointer += (val - 0x60usize) + 2usize;
                },
                _ => pointer +=1,
            }
        }

        Self {gas: U256::from(0), 
            pc: 0,
            memory: Vec::new(),
            active_words: 0,
            stack: Vec::new(),
            return_back: Vec::new(),
            safe_jump: safe_jump,
        }
    }

    pub fn peek(&self, n:usize) -> U256 {
        let index = self.stack.len().checked_sub(n+1).expect("Stack underflow during peak");
        self.stack[index]
    }

    pub fn return_gas(&mut self) -> U256 {
        let gas = self.gas;
        self.gas = U256::ZERO;
        return gas;
    }

}


impl Xi for EVM {
    fn evm_run(&mut self, leviathan: &mut LEVIATHAN, state: &mut WorldState, substate: &mut SubState, execution_environment: &mut ExecutionEnvironment) -> Result<Vec<u8>,Option<Vec<u8>>>  {
        //Ok()：正常停止
        //Err(None) => Z関数による停止
        //Err(Some(Vec<u8>)) => REVERTによる停止

        let code = execution_environment.i_byte.clone();
        let mut opcode = 0u8;

        loop {
            // opcodeを取り出す
            if code.len() <= self.pc{
                opcode = 0x00;      //opcodeをSTOPに
            }else{
                opcode = code[self.pc];
            }
            
            //Z関数による安全性を確認
            if !self.is_safe(opcode, &substate, &state, &execution_environment) {
                return Err(None);      //例外的な停止
            }
    
            //O関数による状態遷移
            let result = self.execution(opcode, leviathan, substate, state, execution_environment);

            if result.is_some() {       //Some(true)：Revert / Some(false):STOP, RETURN, SELFDESTRUCT
                    if result.unwrap() {    //REVERT
                        return Err(Some(self.return_back.clone()));
                    }else{
                        return Ok(self.return_back.clone());
                    }
            }
                        
        }
    }

}


// ↓ 「cargo test」を実行した時だけ、このブロックをコンパイルしてね、という合図です
// ↓ 「cargo test」を実行した時だけ、このブロックをコンパイルしてね、という合図です
#[cfg(test)]
mod tests {
    use super::*; // EVM構造体などを読み込み
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::collections::HashMap;

    // あなたが作った構造体をインポート
    use crate::leviathan::world_state::{WorldState, Account, Address};
    use crate::leviathan::structs::{ExecutionEnvironment, BlockHeader, SubState};
    use crate::my_trait::leviathan_trait::TransactionExecution;
    use crate::leviathan::leviathan::LEVIATHAN;
    
    // パーサーをインポート
    use crate::test::test_parser::VmTestSuite;
    use crate::my_trait::evm_trait::Xi;

    // --- 変更点1: サブディレクトリを再帰的に探索して全JSONを取得するヘルパー関数 ---
    fn get_all_json_files(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if dir.is_dir() {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        // ディレクトリなら再帰的に潜る
                        files.extend(get_all_json_files(&path));
                    } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        // JSONファイルならリストに追加
                        files.push(path);
                    }
                }
            }
        }
        files
    }

    #[test]
    fn test_all_vm_tests() { // 関数名も全体テスト用に変更
        // --- 変更点2: テストのルートディレクトリを指定 ---
        let test_dir_str = "test_data/VMTests";
        let test_dir = Path::new(test_dir_str);
        
        // 全JSONファイルのパスを取得
        let json_files = get_all_json_files(test_dir);

        let mut pass_count = 0;
        let mut total_count = 0;

        for path in json_files {
            total_count += 1;

            let file_name = path.file_name().unwrap().to_str().unwrap();
            // どのカテゴリのテストを実行しているか分かりやすくするため、親ディレクトリ名も取得
            let parent_dir = path.parent().unwrap().file_name().unwrap().to_str().unwrap();
            
            println!("========================================");
            println!("▶ Loading File: {}/{}", parent_dir, file_name);

            let json_data = fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("ファイルの読み込みに失敗しました: {:?}", path));

            let suite: VmTestSuite = serde_json::from_str(&json_data)
                .unwrap_or_else(|_| panic!("JSONのパースに失敗しました: {:?}", path));

            for (test_name, test_data) in suite {
                println!("--- Running Test Case: {} ---", test_name);

                let block_header = BlockHeader {
                    h_beneficiary: Address::new(*test_data.env.current_coinbase.0),
                    h_timestamp: test_data.env.current_timestamp,
                    h_number: test_data.env.current_number,
                    h_prevrandao: test_data.env.current_difficulty,
                    h_gaslimit: test_data.env.current_gas_limit,
                    h_basefee: U256::ZERO,
                };

                let mut execution_environment = ExecutionEnvironment {
                    i_address: Address::new(*test_data.exec.address.0),
                    i_origin: Address::new(*test_data.exec.origin.0),
                    i_gas_price: test_data.exec.gas_price,
                    i_data: test_data.exec.data.to_vec(),
                    i_sender: Address::new(*test_data.exec.caller.0),
                    i_value: test_data.exec.value,
                    i_byte: test_data.exec.code.to_vec(),
                    i_block_header: &block_header,
                    i_depth: 0,
                    i_permission: true,
                };

                let build_initial_state = || {
                    let mut world_state_map = HashMap::new();
                    for (addr, acc_data) in &test_data.pre {
                        let account = Account {
                            nonce: acc_data.nonce.try_into().unwrap_or(0),
                            balance: acc_data.balance,
                            storage: acc_data.storage.clone(),
                            code: acc_data.code.to_vec(),
                        };
                        world_state_map.insert(Address::new(*addr.0), account);
                    }
                    WorldState(world_state_map)
                };

                let mut state = build_initial_state();

                let mut substate = SubState {
                    a_des: Vec::new(),
                    a_log: Vec::new(),
                    a_touch: Vec::new(),
                    a_reimburse: 0,
                    a_access: Vec::new(),
                    a_access_storage: HashMap::new(),
                };

                let mut evm = EVM::new(&execution_environment);
                evm.gas = test_data.exec.gas;

                let mut leviathan = LEVIATHAN::new();
                let result = evm.evm_run(&mut leviathan, &mut state, &mut substate, &mut execution_environment);

                match result {
                    Ok(_) => {
                        let gas_used = test_data.exec.gas.saturating_sub(evm.gas);
                        let max_refund = gas_used / U256::from(2);
                        let raw_refund = U256::from(substate.a_reimburse.max(0) as u64);
                        let actual_refund = std::cmp::min(raw_refund, max_refund);

                        evm.gas = evm.gas.saturating_add(actual_refund);
                    }
                    Err(None) => {
                        state = build_initial_state();
                        evm.gas = U256::ZERO;
                    }
                    Err(Some(_)) => {
                        state = build_initial_state();
                    }
                }

                if let Some(expected_gas) = test_data.gas {
                    assert_eq!(
                        evm.gas,
                        expected_gas,
                        "[{}] ガス計算が間違っています！ (期待値: {}, 実際: {})",
                        test_name, expected_gas, evm.gas
                    );
                    println!("✓ Gas: OK");
                }

                let target_address = Address::new(*test_data.exec.address.0);
                let actual_account = state.0.get(&target_address).expect("アカウントが存在しません");

                if let Some(post_state) = &test_data.post {
                    if let Some(expected_account) = post_state.get(&test_data.exec.address) {
                        for (key, expected_val) in &expected_account.storage {
                            let actual_val = actual_account.storage.get(key).unwrap_or(&U256::ZERO);

                            assert_eq!(
                                actual_val,
                                expected_val,
                                "[{}] スロット {} の値が違います！ (期待値: {}, 実際: {})",
                                test_name, key, expected_val, actual_val
                            );
                        }
                    }
                }
                println!("✓ Storage: OK");
                println!("🎉 Passed: {}", test_name);
            }
        }
        
        println!("========================================");
        println!("🏆 最終結果: {} / {} ファイル内のテストを全てクリアしました！", pass_count, total_count);
    }
}
