#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::evm_trait::{Xi, Gfunction, Zfunction, Hfunction, Ofunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};

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
}


impl Xi for EVM {
    fn evm_run(&mut self, state: &mut WorldState, substate: &mut SubState, execution_environment: &mut ExecutionEnvironment) -> Result<Vec<u8>,Option<Vec<u8>>>  {
        //Ok()：正常停止
        //Err(None) => Z関数による停止
        //Err(Some(Vec<u8>)) => REVERTによる停止

        let code = execution_environment.i_byte.clone();
        let mut opcode = 0u8;

        loop {
            // opcodeを取り出す
            if code.len() < self.pc{
                opcode = 0x00;      //opcodeをSTOPに
            }else{
                opcode = code[self.pc];
            }
            
            //Z関数による安全性を確認
            if !self.is_safe(opcode, &substate, &state, &execution_environment) {
                return Err(None);      //例外的な停止
            }
    
            //O関数による状態遷移
            let result = self.execution(opcode, substate, state, execution_environment);

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


// --- evm.rs の一番下 ---

// ↓ 「cargo test」を実行した時だけ、このブロックをコンパイルしてね、という合図です
#[cfg(test)]
mod tests {
    use super::*; // EVM構造体などを読み込み
    use std::fs;
    use std::path::Path;
    use std::collections::HashMap;

    // あなたが作った構造体をインポート
    use crate::leviathan::world_state::{WorldState, Account, Address};
    use crate::leviathan::structs::{ExecutionEnvironment, BlockHeader, SubState};
    // パーサーをインポート
    use crate::test::test_parser::VmTestSuite;
    use crate::my_trait::evm_trait::Xi;

    #[test]
    fn test_all_arithmetic() {
        // テストファイルが入っているディレクトリのパス
        let test_dir = "test_data/VMTests/vmArithmeticTest/add";
        
        // ディレクトリ内のすべてのファイルを探索
        let paths = fs::read_dir(test_dir).expect("ディレクトリの読み込みに失敗しました");

        let mut pass_count = 0;
        let mut total_count = 0;

        for path in paths {
            let path = path.unwrap().path();
            
            // 拡張子が .json のファイルだけを処理する
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                total_count += 1;
                
                let file_name = path.file_name().unwrap().to_str().unwrap();
                println!("========================================");
                println!("▶ Loading File: {}", file_name);

                let json_data = fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("ファイルの読み込みに失敗しました: {:?}", path));
                
                // 受け皿（パーサー）にパース
                let suite: VmTestSuite = serde_json::from_str(&json_data)
                    .unwrap_or_else(|_| panic!("JSONのパースに失敗しました: {:?}", path));

                // ファイル内のすべてのテストケースを実行
                for (test_name, test_data) in suite {
                    println!("--- Running Test Case: {} ---", test_name);

                    // --- 詰め替え作業 ---
                    let block_header = BlockHeader {
                        h_beneficiary: Address::new(*test_data.env.current_coinbase.0),
                        h_timestamp: test_data.env.current_timestamp,
                        h_number: test_data.env.current_number,
                        h_prevrandao: U256::ZERO,
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
                        i_block_header: block_header,
                        i_depth: 0,
                        i_permission: true,
                    };

                    let mut world_state_map = HashMap::new();
                    for (addr, acc_data) in test_data.pre {
                        let account = Account {
                            nonce: acc_data.nonce.try_into().unwrap_or(0),
                            balance: acc_data.balance,
                            storage: acc_data.storage,
                            code: acc_data.code.to_vec(),
                        };
                        world_state_map.insert(Address::new(*addr.0), account);
                    }
                    let mut state = WorldState(world_state_map);

                    let mut substate = SubState {
                        a_des: Vec::new(),
                        a_log: Vec::new(),
                        a_touch: Vec::new(),
                        a_reimburse: 0,
                        a_access: Vec::new(),
                        a_access_storage: HashMap::new(),
                    };

                    // --- EVMの実行 ---
                    let mut evm = EVM::new(&execution_environment);
                    evm.gas = test_data.exec.gas;

                    let _result = evm.evm_run(&mut state, &mut substate, &mut execution_environment);

                    // --- 検証 (Assertion) ---

                    // 1. ガスの検証
                    if let Some(expected_gas) = test_data.gas {
                        assert_eq!(
                            evm.gas, 
                            expected_gas, 
                            "[{}] ガス計算が間違っています！ (期待値: {}, 実際: {})", 
                            test_name, expected_gas, evm.gas
                        );
                        println!("✓ Gas: OK");
                    }

                    // 2. ストレージの検証 (JSONのpostデータを使って動的に検証)
                    let target_address = Address::new(*test_data.exec.address.0);
                    let actual_account = state.0.get(&target_address).expect("アカウントが存在しません");

                    if let Some(post_state) = &test_data.post {
                        if let Some(expected_account) = post_state.get(&test_data.exec.address) {
                            // JSONに記載されているすべての正解ストレージスロットを検証
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
                    pass_count += 1;
                }
            }
        }
        
        println!("========================================");
        println!("🏆 最終結果: {} / {} ファイル内のテストを全てクリアしました！", pass_count, total_count);
    }
}
