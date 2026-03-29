#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution, TransactionChecks, ContractCreation, RoleBack};
use crate::my_trait::evm_trait::{Xi, Gfunction, Zfunction, Hfunction, Ofunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction, BlockHeader, BackupSubstate};
use crate::evm::evm::EVM;
use sha3::{Keccak256, Digest};
use rlp::RlpStream;
use std::collections::HashMap;


impl ContractCreation for LEVIATHAN {
    fn contract_creation(&mut self, state: &mut WorldState, substate: &mut SubState, sender: Address, origin: Address,
                         gas: U256, price: U256, eth: U256, init_code: Vec<u8>, depth: usize, salt: Option<U256>, sudo: bool, block_header: &BlockHeader
                         ) -> Result<(U256,Vec<u8>),(U256,Option<Vec<u8>>)> {

        //新しいアカウントのアドレス
        let byte: Vec<u8> = if salt.is_none() {
            //CREATE
            let nonce = state.get_nonce(&sender).unwrap() -1;
            let mut stream = RlpStream::new_list(2);
            stream.append(&sender.0.as_ref());
            stream.append(&nonce);
            stream.out().to_vec()
        }else{
            //CREATE2
            let mut tmp = [0u8;85];
            tmp[0] = 0xff;      //定数
            tmp[1..21].copy_from_slice(&sender.0);  //送信者のアドレス
            let salt_byte:[u8;32] = salt.unwrap().to_be_bytes(); 
            tmp[21..53].copy_from_slice(&salt_byte);   //salt
            let mut hasher = Keccak256::new();
            hasher.update(&init_code);
            let result:[u8;32] = hasher.finalize().try_into().unwrap();
            tmp[53..85].copy_from_slice(&result);
            tmp.to_vec()
        };
        let mut hasher = Keccak256::new();
        hasher.update(&byte);
        let result:[u8;32] = hasher.finalize().try_into().unwrap();
        let mut tmp = [0u8;20];
        tmp.copy_from_slice(&result[12..32]);
        let contract_address = Address::new(tmp);

        let nonce = state.get_nonce(&contract_address).unwrap_or(0);
        let code = state.get_code(&contract_address).unwrap_or_else(|| Vec::new());
        let sender_balance = state.get_balance(&sender).unwrap_or(U256::ZERO);

        let is_collision = nonce != 0 || !code.is_empty(); // アドレス衝突
        let is_too_deep = depth >= 1024;                   // 深さ制限
        let is_insufficient_funds = eth > sender_balance;  // 残高不足

        if is_collision || is_too_deep || is_insufficient_funds {
            return Err((U256::ZERO, None));
        }
        
        //サブステートのアクセス済みアカウントに追加
        if !substate.a_access.contains(&contract_address) {
            substate.a_access.push(contract_address.clone())
        }
        self.substate_backup = BackupSubstate::backup(substate); //サブステートのバックアップ

        //Nonceを1にする．
        if state.is_empty(&contract_address) {
            Action::Account_creation(contract_address.clone()).push(self, state);   //アカウントが存在しない場合
        }
        Action::Add_nonce(contract_address.clone()).push(self, state);     //ロールバック用
        state.inc_nonce(&contract_address);
        //送金する
        Action::Send_eth(sender.clone(), contract_address.clone(), eth).push(self, state);     //ロールバック用
        state.send_eth(&sender, &contract_address, eth);
        //storageRootを空にする
        Action::Reset_storage(contract_address.clone(),HashMap::<U256, U256>::new()).push(self, state);     //ロールバック用
        state.reset_storage(&contract_address);
        //codehashに空配列をセット
        Action::Store_code(contract_address.clone(), Vec::new()).push(self, state);     //ロールバック用
        state.set_code(&contract_address, Vec::<u8>::new());

        //Execution Environmentの構築
        let mut execution_environment = ExecutionEnvironment::new(contract_address.clone(),origin.clone(), price, Vec::new(), sender.clone(),eth, 
                                                            init_code, block_header, depth,sudo);

        //仮想マシンの実行
        let mut evm = EVM::new(&execution_environment);
        evm.gas = gas;
        let result = evm.evm_run(self, state, substate, &mut execution_environment);
        //Ok()：正常停止
        //Err(None) => Z関数による停止
        //Err(Some(Vec<u8>)) => REVERTによる停止

        match result {
            Ok(output) => {
                //不正なプレフィックス
                if output.len() > 0 && output[0] == 0xefu8 {
                    self.roleback(state);   //Roleback実行
                    substate.road_backup(self.substate_backup.clone());  //SubStateの巻き戻し
                    return Err((U256::ZERO, None));
                }

                //コードのサイズ制限
                if output.len() > 24576 {
                    self.roleback(state);   //Roleback実行
                    substate.road_backup(self.substate_backup.clone());  //SubStateの巻き戻し
                    return Err((U256::ZERO, None));
                }

                //コードデプロイ費用
                let deposit_gas = 200 * output.len();
                let rest_gas = evm.return_gas();
                if U256::from(deposit_gas) > rest_gas {
                    self.roleback(state);   //Roleback実行
                    substate.road_backup(self.substate_backup.clone());  //SubStateの巻き戻し
                    return Err((U256::ZERO, None));
                }

                //最終処理
                let return_gas = rest_gas - U256::from(deposit_gas);
                state.set_code(&contract_address, output);
                return Ok((return_gas, Vec::<u8>::new()));

            },

            Err(Some(revert_data)) => {
                //REVERT
                let revert_gas = evm.return_gas();  //ガス返却
                self.roleback(state);   //Roleback実行
                substate.road_backup(self.substate_backup.clone());  //SubStateの巻き戻し
                return Err((revert_gas, Some(revert_data)));
            },

            Err(None) => {
                //Z関数による停止
                self.roleback(state);   //Roleback実行
                substate.road_backup(self.substate_backup.clone());  //SubStateの巻き戻し
                return Err((U256::ZERO, None));
            }
        }

    }
}
