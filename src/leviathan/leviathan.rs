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

