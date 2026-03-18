#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::evm_trait::{Xi, Gfunction, Ofunction};
use crate::my_trait::leviathan_trait::State;
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use crate::evm::evm::EVM;
use sha3::{Keccak256, Digest};

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

            0x0a => {   //EXP
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_pow(val2);
                self.push(result);
            },

            0x0b => {   
                let b = self.pop();
                let x = self.pop();
                if b >= U256::from(31) {
                    self.push(x);
                }else{
                    let b_usize:usize = b.try_into().unwrap();

                    let sign_bit_index = (b_usize * 8) +7;
                    let shift_amount = (b_usize + 1) * 8;
                    let mask:U256 = U256::MAX << shift_amount;

                    if x.bit(sign_bit_index) {
                        let result = x | mask;
                        self.push(result);
                    }else{
                        let result = x & !mask;
                        self.push(result);
                    }
                }
            },

            0x10 => {       //LT
                let val1 = self.pop();
                let val2 = self.pop();
                let result = if val1 < val2 {
                    U256::from(1)
                }else{
                    U256::ZERO
                };
                self.push(result);
            },

            0x11 => {       //GT
                let val1 = self.pop();
                let val2 = self.pop();
                let result = if val1 > val2 {
                    U256::from(1)
                }else{
                    U256::ZERO
                };
                self.push(result);
            },

            0x12 => {       //SLT
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                let result = if val1 < val2 {
                    U256::from(1)
                }else{
                    U256::ZERO
                };
                self.push(result);
            }

            0x13 => {       //SGT
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                let result = if val1 > val2 {
                    U256::from(1)
                }else{
                    U256::ZERO
                };
                self.push(result);
            },

            0x14 => {       //EQ
                let val1 = self.pop();
                let val2 = self.pop();
                let result = if val1 == val2 {
                    U256::from(1)
                }else{
                    U256::ZERO
                };
                self.push(result);
            },

            0x15 => {       //ISZERO
                let val1 = self.pop();
                let result = if val1 == U256::ZERO {
                    U256::from(1)
                }else{
                    U256::ZERO
                };
                self.push(result);
            },

            0x16 => {       //AND
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.bitand(val2);
                self.push(result);
            },

            0x17 => {       //OR
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.bitor(val2);
                self.push(result);
            },

            0x18 => {       //XOR
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.bitxor(val2);
                self.push(result);
            },

            0x19 => {       //NOT
                let val1 = self.pop();
                self.push(!val1);
            },

            0x1a => {       //BYTE
                let val1 = self.pop();
                let val2 = self.pop();
                if val1 >= U256::from(32) {
                    self.push(U256::ZERO);
                }else{
                    let val1_usize:usize = val1.try_into().unwrap();
                    let data:[u8;32] = val2.to_be_bytes();
                    let result:u8 = data[val1_usize];
                    self.push(U256::from(result));
                }
            },

            0x1b => {       //SHL
                let val1 = self.pop().try_into().unwrap_or(usize::MAX);
                let val2 = self.pop();
                if val1 >= 256 {
                    self.push(U256::ZERO);
                }else{
                    let result = val2 << val1;
                    self.push(result);
                }
            },

            0x1c => {       //SHR
                let val1 = self.pop().try_into().unwrap_or(usize::MAX);
                let val2 = self.pop();
                if val1 >= 256 {
                    self.push(U256::ZERO);
                }else{
                    let result = val2 >> val1;
                    self.push(result);
                }
            },

            0x1d => {       //SAR
                let val1 = self.pop().try_into().unwrap_or(usize::MAX);
                let val2 = I256::from_raw(self.pop());
                if val1 >= 256 {
                    if val2 >= I256::ZERO {
                        self.push(U256::ZERO);
                    }else if val2 < I256::ZERO{
                        self.push(I256::MINUS_ONE.into_raw());
                    }
                }else{
                    let result = val2.asr(val1);
                    self.push(result.into_raw());
                }
            },

            0x20 => {       //KECCAK256
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let size = self.pop().try_into().unwrap_or(usize::MAX);

                let slice = if size == 0 {
                    &[0u8;0]
                }else{
                    let required_size = offset.saturating_add(size);
                    if required_size > self.memory.len() {
                        let words = (required_size.saturating_add(31))/32;
                        self.memory.resize(words * 32, 0);
                    }
                    &self.memory[offset .. required_size]
                };
                //keccak256準備
                let mut hasher = Keccak256::new();
                hasher.update(slice);
                let result = hasher.finalize().try_into().unwrap();
                let val = U256::from_be_bytes(result);
                self.push(val);
            },

            0x30 => {       //ADDRESS
                let address = &execution_environment.i_address;
                let val = address.to_u256();
                self.push(val);
            },

            0x31 => {       //BALANCE
                let val1 = self.pop();
                let address = Address::from_u256(val1);
                let balance = state.get_balance(&address);
                match balance {
                    Some(x) => self.push(x),
                    None => self.push(U256::ZERO),
                }
            },

            0x32 => {       //ORIGIN
                let address = &execution_environment.i_origin;
                let val = address.to_u256();
                self.push(val);
            },

            0x33 => {       //CALLER
                let address = &execution_environment.i_sender;
                let val = address.to_u256();
                self.push(val);
            },

            0x34 => {       //CALLVALUE
                let val = execution_environment.i_value;
                self.push(val);
            },

            0x35 => {       //CALLDATALOAD
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let data = &execution_environment.i_data;
                let required_size = offset.saturating_add(32);
                let mut buffer = [0u8; 32];
                if offset >= data.len() {
                    self.push(U256::ZERO)
                }else if required_size > data.len() {
                    buffer[..data.len() - offset].copy_from_slice(&data[offset .. data.len()]);
                }else{
                    buffer[..].copy_from_slice(&data[offset .. required_size]);
                }
                let val = U256::from_be_bytes(buffer);
                self.push(val);
            },

            0x36 => {       //CALLDATASIZE
                let data = &execution_environment.i_data;
                self.push(U256::from(data.len()));
            },

            0x37 => {       //CALLDATACOPY
                let data = &execution_environment.i_data;
                let dest_offset = self.pop().try_into().unwrap_or(usize::MAX);
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let size = self.pop().try_into().unwrap_or(usize::MAX);
                //メモリ拡張
                if size != 0 {
                    let required_size = dest_offset.saturating_add(size);
                    if required_size > self.memory.len() {
                        let words = (required_size.saturating_add(31))/32;
                        self.memory.resize(words * 32, 0);
                    }
                    //メモリに値を書き込む
                    let read_size = offset.saturating_add(size);
                    if offset <= data.len() {
                        if read_size > data.len() {
                            let copy_len =  data.len() - offset;
                            self.memory[dest_offset .. dest_offset + copy_len].copy_from_slice(&data[offset .. data.len()]);
                        }else{
                            self.memory[dest_offset .. required_size].copy_from_slice(&data[offset .. read_size]);
                        }
                    }
                }
            },

            0x38 => {       //CODESIZE
                let size = execution_environment.i_byte.len();
                self.push(U256::from(size));
            },

            0x39 => {       //CODECOPY
                let data = &execution_environment.i_byte;
                let dest_offset = self.pop().try_into().unwrap_or(usize::MAX);
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let size = self.pop().try_into().unwrap_or(usize::MAX);
                //メモリ拡張
                if size != 0 {
                    let required_size = dest_offset.saturating_add(size);
                    if required_size > self.memory.len() {
                        let words = (required_size.saturating_add(31))/32;
                        self.memory.resize(words * 32, 0);
                    }
                    //メモリに値を書き込む
                    let read_size = offset.saturating_add(size);
                    if offset <= data.len() {
                        if read_size > data.len() {
                            let copy_len =  data.len() - offset;
                            self.memory[dest_offset .. dest_offset + copy_len].copy_from_slice(&data[offset .. data.len()]);
                        }else{
                            self.memory[dest_offset .. required_size].copy_from_slice(&data[offset .. read_size]);
                        }
                    }
                }
            },

            0x3a => {       //GASPRICE
                let price = execution_environment.i_gas_price;
                self.push(price);
            },

            0x3b => {       //EXTCODESIZE
                let val1 = self.pop();
                let address = Address::from_u256(val1);
                let result = state.get_code(&address);
                match result {
                    Some(x) => self.push(U256::from(x.len())),
                    None => self.push(U256::ZERO),
                }
            },

            0x3c => {       //EXTCODECOPY
                let val1 = self.pop();
                let address = Address::from_u256(val1);
                let dest_offset = self.pop().try_into().unwrap_or(usize::MAX);      //メモリ
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let size = self.pop().try_into().unwrap_or(usize::MAX);
                //コード取得
                let result = state.get_code(&address);
                let code = match result {
                    Some(x) => x,
                    None => Vec::<u8>::new(),
                };
                //メモリ拡張
                if size != 0 {
                    let required_size = dest_offset.saturating_add(size);
                    if required_size > self.memory.len() {
                        let words = (required_size.saturating_add(31))/32;
                        self.memory.resize(words * 32, 0);
                    }
                    //メモリに値を書き込む
                    let read_size = offset.saturating_add(size);
                    if offset <= code.len() {
                        if read_size > code.len() {
                            let copy_len =  code.len() - offset;
                            self.memory[dest_offset .. dest_offset + copy_len].copy_from_slice(&code[offset .. code.len()]);
                        }else{
                            self.memory[dest_offset .. required_size].copy_from_slice(&code[offset .. read_size]);
                        }
                    }
                }
            },

            0x3d => {   //RETURNDATASIZE
                let size = self.return_back.len();
                self.push(U256::from(size));
            },











                




            _ => todo!(),
        }



    }
}

