#![allow(dead_code)]

use primitive_types::U256; 
use crate::my_trait::evm_trait::{Xi, Gfunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};

pub struct EVM {
    gas: U256,
    pc: usize,
    memory: Vec<u8>,
    active_words: usize,
    stack: Vec<U256>,
    return_back: Vec<u8>
}


impl EVM {
    pub fn new() -> Self {
        Self {gas: U256::from(0), 
            pc: 0,
            memory: Vec::new(),
            active_words: 0,
            stack: Vec::new(),
            return_back: Vec::new()
        }
    }
}


impl Xi for EVM {
    fn evm_run(&mut self, state: WorldState, substate: SubState, execution_environment: ExecutionEnvironment) -> Result<(WorldState, SubState, ExecutionEnvironment, Vec<u8>), (WorldState, SubState, ExecutionEnvironment, Vec<u8>)>  {

        let code = execution_environment.i_byte.clone();
        let mut opcode = 0u8;

        loop {
            //==============Opcode を取り出す================
            if code.len() <= self.pc {
                opcode = 0x00   //STOP
            }
            opcode = code[self.pc];


            return Ok((state, substate, execution_environment, Vec::new()))
        }
    }

}

