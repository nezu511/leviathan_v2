#![allow(dead_code)]

use primitive_types::U256; 
use crate::my_trait::evm_trait::{Xi, Gfunction, Ofunction};
use crate::my_trait::leviathan_trait::State;
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use crate::evm::evm::EVM;

impl Ofunction for EVM {
    fn execution(&mut self, opcode:u8, substate: &mut SubState, state: &mut WorldState, execution_environment: &ExecutionEnvironment) {
        //ガスを消費
        let gas_cost = self.gas(opcode, substate, state, execution_environment);
        self.gas - gas_cost;

        //プログラムカウンターを進める
    }
}

