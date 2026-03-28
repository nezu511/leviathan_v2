use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction};
use alloy_primitives::{I256, U256};

pub trait State {
    fn is_empty(&self,address: &Address) -> bool;   //空だとtrue;
                                            
    fn get_balance(&self, address: &Address) -> Option<U256>;

    fn get_code(&self, address: &Address) -> Option<Vec<u8>>;

    fn get_storage_value(&self, address: &Address, key: &U256) -> Option<U256>;
    
    // 書き込み系
    fn set_balance(&mut self, address: &Address, value:U256);

    fn inc_nonce(&mut self, address: &Address);

    fn set_storage(&mut self, address: &Address, key: U256, value: U256);

    fn set_code(&mut self, address: &Address, code: Vec<u8>);
    
    fn remove_storage(&mut self, address: &Address, key:U256) ;

    fn send_eth(&mut self, from: &Address, to: &Address, eth:U256) -> Result<(),&'static str>;

    fn buy_gas(&mut self, address: &Address, limit: U256, price: U256) -> Result<U256,&'static str>;

    fn reset_storage(&mut self, address: &Address);

    fn delete_account(&mut self, address: &Address);

    fn add_account(&mut self, address: &Address, account: Account);

    fn reset_balance(&mut self, address: &Address);


}

pub trait TransactionExecution {
     fn execution(state: &mut WorldState, transaction:Transaction) -> Result<(U256, Vec<Log>, bool),(U256, Vec<Log>, bool)>;
    //
    // fn contract_creation() -> Result<(WorldState), (WorldState)>;
    //
    // fn message_call() -> Result<(WorldState), (WorldState)>;      
    //
    // fn role_back();      contract_creationもしくはmessage_callの返り値が失敗なら発動！！
}
