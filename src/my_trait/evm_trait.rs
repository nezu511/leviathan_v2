use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use alloy_primitives::{I256, U256};


pub trait Xi {
    fn evm_run(&mut self, state: WorldState, substate: SubState, execution_environment: ExecutionEnvironment) -> Result<(WorldState, SubState, ExecutionEnvironment, Vec<u8>), (WorldState, SubState, ExecutionEnvironment, Option<Vec<u8>>)> ;
}

pub trait Gfunction {
    //返り値は消費ガス量
    fn gas(&mut self, opcode:u8, substate: &SubState, state: &WorldState, execution_environment: &ExecutionEnvironment) -> U256 ;

    fn extension_cost(&mut self, offset:U256, size:U256) -> U256;

    fn is_account_access(&mut self,data: U256, substate: &SubState) -> U256;
}


pub trait Zfunction {
    //Z関数による安全性を確認
    fn is_safe(&mut self, opcode:u8, substate: &SubState, state: &WorldState, execution_environment: &ExecutionEnvironment) -> bool ;
}

pub trait Ofunction {
    //状態遷移
    fn execution(&mut self, opcode:u8, substate: &mut SubState, state: &mut WorldState, execution_environment: &ExecutionEnvironment);

    fn pop(&mut self) -> U256;
    fn push(&mut self, val:U256);
}

pub trait Hfunction {
    fn evm_stop(&mut self, opcode:u8) -> Result<(), Option<Vec<u8>>>;
}
    
