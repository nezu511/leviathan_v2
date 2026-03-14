use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use primitive_types::U256;


pub trait Xi {
    fn evm_run(&mut self, state: WorldState, substate: SubState, execution_environment: ExecutionEnvironment) -> Result<(WorldState, SubState, ExecutionEnvironment, Vec<u8>), (WorldState, SubState, ExecutionEnvironment, Vec<u8>)> ;
}

pub trait Gfunction {
    //返り値は消費ガス量
    fn gas(&mut self, opcode:u8, execution_environment: ExecutionEnvironment) -> U256 ;
    fn extension_cost(&mut self, offset:usize, size:usize) -> usize ;
}


pub trait Zfunction {
    //Z関数による安全性を確認
    fn is_safe(&mut self, opcode:u8, execution_environment: ExecutionEnvironment) -> bool ;
}
