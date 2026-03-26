use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction};
use alloy_primitives::{I256, U256};

pub trait State {
    fn is_empty(&self,address: &Address) -> bool;   //空だとtrue;
                                            
    fn get_balance(&self, address: &Address) -> Option<U256>;

    fn get_code(&self, address: &Address) -> Option<Vec<u8>>;

    fn get_storage_value(&self, address: &Address, key: &U256) -> Option<U256>;

    fn get_nonce(&self, address: &Address) -> Option<u32>;
    
    // 書き込み系
    fn set_balance(&mut self, address: &Address, value:U256);

    fn inc_nonce(&mut self, address: &Address);

    fn set_storage(&mut self, address: &Address, key: U256, value: U256);

    fn set_code(&mut self, address: &Address, code: Vec<u8>);
    
    fn remove_storage(&mut self, address: &Address, key:U256) ;

    fn send_eth(&mut self, from: &Address, to: &Address, eth:U256) -> Result<(),&'static str>;

    //fn delete_account(&mut self, address: &Address);

}

pub trait TransactionChecks {
     fn transaction_checks(state: &mut WorldState, transaction:&Transaction, inti_gas: &U256, pre_cost: &U256) -> Result<Address,&'static str>;
}


pub trait TransactionExecution {
     fn execution(&self, state: &mut WorldState, transaction:Transaction) -> Result<(U256, Vec<Log>, bool),(U256, Vec<Log>, bool)>;


    //
    // fn contract_creation() -> Result<(WorldState), (WorldState)>;
    //
    // fn message_call() -> Result<(WorldState), (WorldState)>;      
    //
    // fn role_back();      contract_creationもしくはmessage_callの返り値が失敗なら発動！！
}
