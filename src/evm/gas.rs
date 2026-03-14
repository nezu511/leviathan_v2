#![allow(dead_code)]

use primitive_types::U256; 
use crate::my_trait::evm_trait::{Xi, Gfunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use crate::evm::evm::EVM;

impl Gfunction for EVM {
    fn gas(&mut self, opcode:u8, execution_environment: ExecutionEnvironment) -> U256 {
        todo!()
    }
}
