use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
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

    //fn delete_account(&mut self, address: &Address);

}

pub trait TransactionExecution {
    // fn execution() -> WorldState;
    //
    // fn contract_creation() -> Result<(WorldState), (WorldState)>;
    //
    // fn message_call() -> Result<(WorldState), (WorldState)>;      
    //
    // fn role_back();      contract_creationもしくはmessage_callの返り値が失敗なら発動！！
}
