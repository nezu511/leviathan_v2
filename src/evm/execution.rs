#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::evm_trait::{Xi, Gfunction, Ofunction};
use crate::my_trait::leviathan_trait::State;
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use crate::evm::evm::EVM;

impl Ofunction for EVM {
    
    fn pop(&mut self) -> U256 {
        self.stack.pop().unwrap()
    }

    fn push(&mut self, val:U256) {
        self.stack.push(val);
    }


    fn execution(&mut self, opcode:u8, substate: &mut SubState, state: &mut WorldState, execution_environment: &ExecutionEnvironment) {
        //ガスを消費
        let gas_cost = self.gas(opcode, substate, state, execution_environment);
        self.gas - gas_cost;

        //プログラムカウンターを進める
        if opcode == 0x56 { //JUMP
            self.pc = self.pop().try_into().unwrap_or(usize::MAX);
        }else if opcode == 0x57 {
            let destination = self.pop().try_into().unwrap_or(usize::MAX);
            let flag = self.pop().try_into().unwrap_or(usize::MAX);
            if flag != 0 {
                self.pc = destination;
            }else{
                self.pc += 1;
            }
        }else{
            match opcode {
                0x60 ..= 0x7F => { 
                    self.pc = self.pc + opcode as usize - 0x5E;
                },
                _ => self.pc += 1,
            }
        }

        //Opcode実践
        match opcode {
            0x01 => {       //ADD
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_add(val2);
                self.push(result);
            },

            0x02 => {       //MUL
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_mul(val2);
                self.push(result);
            },

            0x03 => {       //SUB
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_sub(val2);
                self.push(result);
            },

            0x04 => {       //DIV
                let val1 = self.pop();
                let val2 = self.pop();
                if val2.is_zero() {
                    self.push(U256::ZERO);
                }else{
                    let result = val1.wrapping_div(val2);
                    self.push(result);
                }
            },

            0x05 => {       //SDIV 符号付き!
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                if val2.is_zero() {
                    self.push(U256::ZERO);
                }else if val1 == I256::MIN && val2 == I256::MINUS_ONE {
                    self.push(I256::MIN.into_raw());
                }else{
                    let result = val1.wrapping_div(val2);
                    self.push(result.into_raw());
                }
            },

            0x06 => {       //MOD
                let val1 = self.pop();
                let val2 = self.pop();
                if val2.is_zero() {
                    self.push(U256::ZERO);
                }else{
                    let result = val1.wrapping_rem(val2);
                    self.push(result);
                }
            },

            0x07 => {       //SMOD 符号付き!
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                if val2.is_zero() {
                    self.push(U256::ZERO);
                }else{
                    let result = val1.wrapping_rem(val2);
                    self.push(result.into_raw());
                }
            },

            0x08 => {       //ADDMOD
                let val1 = self.pop();
                let rhs = self.pop();
                let modulus = self.pop();
                let result = val1.add_mod(rhs, modulus);
                self.push(result);
            },

            0x09 => {       //MULMOD
                let val1 = self.pop();
                let rhs = self.pop();
                let modulus = self.pop();
                let result = val1.mul_mod(rhs, modulus);
                self.push(result);
            },






            _ => todo!(),
        }



    }
}

