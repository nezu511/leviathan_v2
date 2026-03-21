#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::evm_trait::{Xi, Gfunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};

pub struct EVM {
    pub gas: U256,
    pub pc: usize,
    pub memory: Vec<u8>,
    pub active_words: usize,
    pub stack: Vec<U256>,
    pub return_back: Vec<u8>,
    pub safe_jump: Vec<u8>,
}


impl EVM {
    pub fn new(execution_environment: &ExecutionEnvironment) -> Self {

        //安全なjump先のリストを作成
        let code = execution_environment.i_byte.clone();
        let code_len = code.len();
        let mut safe_jump:Vec<u8> = vec![0;code_len];
        let mut pointer = 0usize;
        while pointer < code_len {
            let x = code[pointer];
            match x {
                0x5b => {
                    safe_jump[pointer] = 1;
                    pointer += 1;
                },
                0x60 ..=0x7f => {
                    let val = x as usize;
                    pointer += (val - 0x60usize) + 2usize;
                },
                _ => pointer +=1,
            }
        }

        Self {gas: U256::from(0), 
            pc: 0,
            memory: Vec::new(),
            active_words: 0,
            stack: Vec::new(),
            return_back: Vec::new(),
            safe_jump: safe_jump,
        }
    }

    pub fn peek(&self, n:usize) -> U256 {
        let index = self.stack.len().checked_sub(n+1).expect("Stack underflow during peak");
        self.stack[index]
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

