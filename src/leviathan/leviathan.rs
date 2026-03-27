#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution, TransactionChecks};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction};
use crate::evm::evm::EVM;
use sha3::{Keccak256, Digest};

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
     fn execution(&self, state: &mut WorldState, transaction:Transaction) -> Result<(U256, Vec<Log>, bool),(U256, Vec<Log>, bool)> {

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
         let sender_address = LEVIATHAN::transaction_checks(state, &transaction, &all_gas, &max_cost);
         if sender_address.is_err() {
             return Err((U256::ZERO, Vec::new(), true));
         }
         let sender_address = sender_address.unwrap();

         //=======ステップ2===========
         //【Nonceの加算】
         state.inc_nonce(&sender_address);
         //【前払いガス代の徴収】
         let gas = state.buy_gas(&sender_address, transaction.t_price, transaction.t_value);
         //ここからロールバックの起点:ロールバックが起きたらこの状態にする
    
         //=======ステップ3===========
         let result = if transaction.t_to.is_none() {
             //self.contract_creation()
         }else{
             //self.message_call()

         };

         return Ok((U256::ZERO, Vec::new(), true));


         
     }
}

