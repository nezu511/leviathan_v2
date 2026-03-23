#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::evm_trait::{Xi, Gfunction, Ofunction, Hfunction};
use crate::my_trait::leviathan_trait::State;
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log};
use crate::evm::evm::EVM;

impl Hfunction for EVM {
    fn evm_stop(&mut self, opcode:u8) -> Result<(), Option<Vec<u8>>> {
        match opcode {
            0x00 | 0xFF => return Err(None),
            0xF3 | 0xFD => {
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let size = self.pop().try_into().unwrap_or(usize::MAX);
                //メモリ読み取り
                let mut data = Vec::<u8>::new();
                if size > 0 {
                    let required_size = offset.saturating_add(size);
                    if required_size > self.memory.len() {
                        let words = required_size.saturating_add(31) / 32;
                        self.memory.resize(words.saturating_mul(32), 0);
                    }
                    let slice = &self.memory[offset .. required_size];
                    data = slice.to_vec();
                }
                return Err(Some(data));
            },
            _ => return Ok(()),
        }
    }
}
            


