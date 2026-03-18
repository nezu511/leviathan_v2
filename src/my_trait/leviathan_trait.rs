use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use alloy_primitives::{I256, U256};

pub trait State {
    fn is_empty(&self,address: &Address) -> bool;   //空だとtrue;
                                            
    fn get_balance(&self, address: &Address) -> Option<U256>;

    fn get_code(&self, address: &Address) -> Option<Vec<u8>>;

    fn get_storage_value(&self, address: &Address, key: &U256) -> Option<U256>;
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
