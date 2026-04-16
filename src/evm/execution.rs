#![allow(dead_code)]

use crate::evm::evm::EVM;
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{ExecutionEnvironment, Log, SubState, VersionId};
use crate::leviathan::world_state::{Account, WorldState};
use crate::my_trait::evm_trait::{Gfunction, Ofunction};
use crate::my_trait::leviathan_trait::{ContractCreation, MessageCall, State};
use alloy_primitives::{I256, U256, hex, Address, B256};
use sha3::{Digest, Keccak256};

impl Ofunction for EVM {
    fn pop(&mut self) -> U256 {
        self.stack.pop().unwrap()
    }

    fn push(&mut self, val: U256) {
        self.stack.push(val);
    }

    fn execution(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) -> Option<bool> {
        //ガスを消費
        let gas_cost = self.gas(opcode, substate, state, execution_environment);
        self.gas = self.gas.saturating_sub(gas_cost);

        //プログラムカウンターを進める
        if opcode == 0x56 {
            //JUMP
            self.pc = self.pop().try_into().unwrap_or(usize::MAX);
        } else if opcode == 0x57 {
            let destination = self.pop().try_into().unwrap_or(usize::MAX);
            let flag = self.pop().try_into().unwrap_or(usize::MAX);
            if flag != 0 {
                self.pc = destination;
            } else {
                self.pc += 1;
            }
        } else {
            match opcode {
                0x60..=0x7F => {
                    self.pc = self.pc + opcode as usize - 0x5E;
                }
                _ => self.pc += 1,
            }
        }

        //Opcode実践
        match opcode {
            0x00 => {
                //STOP
                return Some(false);
            }

            0x01..=0x0b => {
                self.arithmetic_opcodes(opcode, leviathan, substate, state, execution_environment)
            }

            0x10..=0x1d => self.comparison_bitwise_opcodes(
                opcode,
                leviathan,
                substate,
                state,
                execution_environment,
            ),

            0x20 => {
                self.keccak256_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0x30..=0x34 | 0x36 | 0x38 | 0x3a | 0x3b | 0x3d | 0x3f => self
                .environmental_info_opcode(
                    opcode,
                    leviathan,
                    substate,
                    state,
                    execution_environment,
                ),

            0x35 => {
                self.calldataload_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0x37 => {
                self.calldatacopy_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0x39 => self.codecopy_opcode(opcode, leviathan, substate, state, execution_environment),

            0x3c => {
                self.extcodecopy_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0x3e => self.returndatacopy_opcode(
                opcode,
                leviathan,
                substate,
                state,
                execution_environment,
            ),

            0x40..=0x48 => {
                self.block_info_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0x50 => {
                self.pop();
            }

            0x51 => self.mload_opcode(opcode, leviathan, substate, state, execution_environment),

            0x52 => self.mstore_opcode(opcode, leviathan, substate, state, execution_environment),

            0x53 => self.mstore8_opcode(opcode, leviathan, substate, state, execution_environment),

            0x54 => self.sload_opcode(opcode, leviathan, substate, state, execution_environment),

            0x55 => self.sstore_opcode(opcode, leviathan, substate, state, execution_environment),

            0x56 | 0x57 => (),

            0x58 => {
                //PC
                self.push(U256::from(self.pc - 1));
            }

            0x59 => {
                //MSIZE
                let len = self.memory.len();
                self.push(U256::from(len));
            }

            0x5a => {
                //GAS
                self.push(self.gas);
            }

            0x5b => { //JUMPDEST
            }

            0x5f => {
                //push0
                self.push(U256::ZERO);
            }

            0x60..=0x7f => {
                self.push_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0x80..=0x8f => {
                //DUP
                let n = opcode as usize - 0x80;
                let data = self.peek(n);
                self.push(data);
            }

            0x90..=0x9f => {
                //SWAP
                let n = opcode as usize - 0x90 + 1;
                let top = self.stack.len() - 1;
                let target = self.stack.len() - 1 - n;
                self.stack.swap(top, target);
            }

            0xa0..=0xa4 => {
                self.log_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0xf0 => self.create_opcode(opcode, leviathan, substate, state, execution_environment),

            0xf1 => self.call_opcode(opcode, leviathan, substate, state, execution_environment),

            0xf2 => self.callcode_opcode(opcode, leviathan, substate, state, execution_environment),

            0xf4 => {
                self.delegatecall_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0xf5 => self.create2_opcode(opcode, leviathan, substate, state, execution_environment),

            0xf3 => {
                //RETURN
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let size = self.pop().try_into().unwrap_or(usize::MAX);
                //メモリ読み取り
                if size > 0 {
                    let required_size = offset.saturating_add(size);
                    if required_size > self.memory.len() {
                        let words = required_size.saturating_add(31) / 32;
                        self.memory.resize(words.saturating_mul(32), 0);
                    }
                    let slice = &self.memory[offset..required_size];
                    self.return_back = slice.to_vec();
                }
                //アクティブなword数を更新
                let active_words = self.memory.len() / 32;
                self.active_words = active_words;
                return Some(false);
            }

            0xfa => {
                self.staticcall_opcode(opcode, leviathan, substate, state, execution_environment)
            }

            0xfd => {
                //REVERT
                let offset = self.pop().try_into().unwrap_or(usize::MAX);
                let size = self.pop().try_into().unwrap_or(usize::MAX);
                //メモリ読み取り
                if size > 0 {
                    let required_size = offset.saturating_add(size);
                    if required_size > self.memory.len() {
                        let words = required_size.saturating_add(31) / 32;
                        self.memory.resize(words.saturating_mul(32), 0);
                    }
                    let slice = &self.memory[offset..required_size];
                    self.return_back = slice.to_vec();
                }
                //アクティブなword数を更新
                let active_words = self.memory.len() / 32;
                self.active_words = active_words;
                return Some(true);
            }

            0xff => {
                //SELFDESTRUCT
                let from_address = &execution_environment.i_address;
                if self.version < VersionId::London && !substate.a_des.contains(from_address) {
                    substate.a_reimburse += 24000;
                }
                let val1 = self.pop();
                let to_address = Address::from_word(B256::from(val1.to_be_bytes::<32>()));
                //デバック用
                tracing::info!(
                    recipient = format_args!("0x{}", hex::encode(to_address.0)),
                    "SELFDESTRUCT"
                );
                let balance = state.get_balance(from_address).unwrap();
                if from_address.clone() == to_address {
                    Action::ResetBalance(from_address.clone(), U256::ZERO).push(leviathan, state); //ロールバック用
                    state.reset_balance(from_address)
                } else {
                    if balance != U256::ZERO {
                        if state.is_empty(from_address) {
                            return Some(false);
                        }
                        if state.is_empty(&to_address) && !state.is_physically_exist(&to_address) {
                            state.add_account(&to_address, Account::new()); //アカウントを追加
                            Action::AccountCreation(to_address.clone()).push(leviathan, state); //アカウントが存在しない場合
                        }
                        Action::SendEth(from_address.clone(), to_address.clone(), balance)
                            .push(leviathan, state); //ロールバック用
                        state.send_eth(from_address, &to_address, balance);
                    }
                }
                substate.a_des.push(from_address.clone());
                return Some(false);
            }

            _ => todo!(),
        }
        None
    }

    #[inline(never)]
    fn call_opcode(
        &mut self,
        _opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //CALL
        let _gas = self.pop(); //サブコールに割り当てる最大ガス
        let to = self.pop(); //呼び出し先のアドレス
        let to_address = Address::from_word(B256::from(to.to_be_bytes::<32>()));
        let value = self.pop();
        let in_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let in_size = self.pop().try_into().unwrap_or(usize::MAX);
        let out_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let out_size = self.pop().try_into().unwrap_or(usize::MAX);
        //メモリ拡張
        let in_end = if in_size == 0 {
            0
        } else {
            in_offset.saturating_add(in_size)
        };
        let out_end = if out_size == 0 {
            0
        } else {
            out_offset.saturating_add(out_size)
        };
        let max_end = in_end.max(out_end);
        let mut data = Vec::<u8>::new();
        if max_end > self.memory.len() {
            let words = max_end.saturating_add(31) / 32;
            self.memory.resize(words.saturating_mul(32), 0);
            let active_words = self.memory.len() / 32; //アクティブなword数を更新
            self.active_words = active_words;
        }
        if in_size > 0 {
            let required_size = in_offset.saturating_add(in_size);
            let slice = &self.memory[in_offset..required_size];
            data = slice.to_vec();
        } else {
            data = Vec::<u8>::new();
        }
        //アクセス済みリストの更新
        if !substate.a_access.contains(&to_address) {
            substate.a_access.push(to_address.clone())
        }
        //子に渡すガスの計算
        let Some(mut child_gas) = self.child_gas_mem else {
            self.push(U256::ZERO);
            return;
        };
        if value > 0 {
            //最終的な子に渡すガス
            child_gas = child_gas.saturating_add(U256::from(2300)); //送金額が0よりも大きい
        } else {
            child_gas;
        }
        self.child_gas_mem = None;
        //デバック出力
        tracing::info!(
        address =  format_args!("0x{}", hex::encode(to_address.0)),
        value = %value,
        data = %hex::encode(&data),
        gas = %child_gas,
        "CALL"
        );
        //事前チェック
        //・残高チェック
        //・コールスタック深度
        let my_balance = state
            .get_balance(&execution_environment.i_address)
            .unwrap_or(U256::from(0));
        let is_balance = my_balance < value; //残高チェック
        let is_deepth = execution_environment.i_depth >= 1024;
        if is_balance || is_deepth {
            self.gas += child_gas;
            self.child_gas_mem = None;
            self.push(U256::ZERO);
            tracing::warn!("[CALL] 事前チェックで例外停止");
            return;
        }
        //depthのインクリメント
        let depth = execution_environment.i_depth + 1;

        //サブコールの実行
        let mut child_leviathan = Box::new(LEVIATHAN::new(self.version));
        let result = child_leviathan.message_call(
            state,
            substate,
            execution_environment.i_address.clone(),
            execution_environment.i_origin.clone(),
            to_address.clone(),
            to_address.clone(),
            child_gas,
            execution_environment.i_gas_price,
            value,
            value,
            data,
            depth,
            execution_environment.i_permission,
            execution_environment.i_block_header,
        );

        //実行後の処理
        match result {
            Ok((return_gas, return_data, _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                tracing::info!(
                return_gas = %return_gas,
                "[CALL] normal end:"
                );
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //Journalのmerge
                leviathan.merge(*child_leviathan);
                //結果push
                self.push(U256::from(1));
            }
            Err((return_gas, Some(return_data), _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //結果push
                self.push(U256::ZERO);
            }

            Err((_return_gas, None, _)) => {
                //結果push
                self.push(U256::ZERO);
            }
        }
    }

    #[inline(never)]
    fn arithmetic_opcodes(
        &mut self,
        opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        match opcode {
            0x01 => {
                //ADD
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_add(val2);
                self.push(result);
            }

            0x02 => {
                //MUL
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_mul(val2);
                self.push(result);
            }

            0x03 => {
                //SUB
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_sub(val2);
                self.push(result);
            }

            0x04 => {
                //DIV
                let val1 = self.pop();
                let val2 = self.pop();
                if val2.is_zero() {
                    self.push(U256::ZERO);
                } else {
                    let result = val1.wrapping_div(val2);
                    self.push(result);
                }
            }

            0x05 => {
                //SDIV 符号付き!
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                if val2.is_zero() {
                    self.push(U256::ZERO);
                } else if val1 == I256::MIN && val2 == I256::MINUS_ONE {
                    self.push(I256::MIN.into_raw());
                } else {
                    let result = val1.wrapping_div(val2);
                    self.push(result.into_raw());
                }
            }

            0x06 => {
                //MOD
                let val1 = self.pop();
                let val2 = self.pop();
                if val2.is_zero() {
                    self.push(U256::ZERO);
                } else {
                    let result = val1.wrapping_rem(val2);
                    self.push(result);
                }
            }

            0x07 => {
                //SMOD 符号付き!
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                if val2.is_zero() {
                    self.push(U256::ZERO);
                } else {
                    let result = val1.wrapping_rem(val2);
                    self.push(result.into_raw());
                }
            }

            0x08 => {
                //ADDMOD
                let val1 = self.pop();
                let rhs = self.pop();
                let modulus = self.pop();
                let result = val1.add_mod(rhs, modulus);
                self.push(result);
            }

            0x09 => {
                //MULMOD
                let val1 = self.pop();
                let rhs = self.pop();
                let modulus = self.pop();
                let result = val1.mul_mod(rhs, modulus);
                self.push(result);
            }

            0x0a => {
                //EXP
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.wrapping_pow(val2);
                self.push(result);
            }

            0x0b => {
                let b = self.pop();
                let x = self.pop();
                if b >= U256::from(31) {
                    self.push(x);
                } else {
                    let b_usize: usize = b.try_into().unwrap();

                    let sign_bit_index = (b_usize * 8) + 7;
                    let shift_amount = (b_usize + 1) * 8;
                    let mask: U256 = U256::MAX << shift_amount;

                    if x.bit(sign_bit_index) {
                        let result = x | mask;
                        self.push(result);
                    } else {
                        let result = x & !mask;
                        self.push(result);
                    }
                }
            }
            0_u8 | 12_u8..=u8::MAX => todo!(),
        }
    }

    #[inline(never)]
    fn comparison_bitwise_opcodes(
        &mut self,
        opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        match opcode {
            0x10 => {
                //LT
                let val1 = self.pop();
                let val2 = self.pop();
                let result = if val1 < val2 {
                    U256::from(1)
                } else {
                    U256::ZERO
                };
                self.push(result);
            }

            0x11 => {
                //GT
                let val1 = self.pop();
                let val2 = self.pop();
                let result = if val1 > val2 {
                    U256::from(1)
                } else {
                    U256::ZERO
                };
                self.push(result);
            }

            0x12 => {
                //SLT
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                let result = if val1 < val2 {
                    U256::from(1)
                } else {
                    U256::ZERO
                };
                self.push(result);
            }

            0x13 => {
                //SGT
                let val1 = I256::from_raw(self.pop());
                let val2 = I256::from_raw(self.pop());
                let result = if val1 > val2 {
                    U256::from(1)
                } else {
                    U256::ZERO
                };
                self.push(result);
            }

            0x14 => {
                //EQ
                let val1 = self.pop();
                let val2 = self.pop();
                let result = if val1 == val2 {
                    U256::from(1)
                } else {
                    U256::ZERO
                };
                self.push(result);
            }

            0x15 => {
                //ISZERO
                let val1 = self.pop();
                let result = if val1 == U256::ZERO {
                    U256::from(1)
                } else {
                    U256::ZERO
                };
                self.push(result);
            }

            0x16 => {
                //AND
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.bitand(val2);
                self.push(result);
            }

            0x17 => {
                //OR
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.bitor(val2);
                self.push(result);
            }

            0x18 => {
                //XOR
                let val1 = self.pop();
                let val2 = self.pop();
                let result = val1.bitxor(val2);
                self.push(result);
            }

            0x19 => {
                //NOT
                let val1 = self.pop();
                self.push(!val1);
            }

            0x1a => {
                //BYTE
                let val1 = self.pop();
                let val2 = self.pop();
                if val1 >= U256::from(32) {
                    self.push(U256::ZERO);
                } else {
                    let val1_usize: usize = val1.try_into().unwrap();
                    let data: [u8; 32] = val2.to_be_bytes();
                    let result: u8 = data[val1_usize];
                    self.push(U256::from(result));
                }
            }

            0x1b => {
                //SHL
                let val1 = self.pop().try_into().unwrap_or(usize::MAX);
                let val2 = self.pop();
                if val1 >= 256 {
                    self.push(U256::ZERO);
                } else {
                    let result = val2 << val1;
                    self.push(result);
                }
            }

            0x1c => {
                //SHR
                let val1 = self.pop().try_into().unwrap_or(usize::MAX);
                let val2 = self.pop();
                if val1 >= 256 {
                    self.push(U256::ZERO);
                } else {
                    let result = val2 >> val1;
                    self.push(result);
                }
            }

            0x1d => {
                //SAR
                let val1 = self.pop().try_into().unwrap_or(usize::MAX);
                let val2 = I256::from_raw(self.pop());
                if val1 >= 256 {
                    if val2 >= I256::ZERO {
                        self.push(U256::ZERO);
                    } else if val2 < I256::ZERO {
                        self.push(I256::MINUS_ONE.into_raw());
                    }
                } else {
                    let result = val2.asr(val1);
                    self.push(result.into_raw());
                }
            }
            0_u8..=15_u8 | 30_u8..=u8::MAX => todo!(),
        }
    }

    #[inline(never)]
    fn keccak256_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        //KECCAK256
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let size = self.pop().try_into().unwrap_or(usize::MAX);

        let slice = if size == 0 {
            &[0u8; 0]
        } else {
            //メモリ拡張
            let required_size = offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = (required_size.saturating_add(31)) / 32;
                self.memory.resize(words * 32, 0);
            }
            &self.memory[offset..required_size]
        };
        //keccak256準備
        let mut hasher = Keccak256::new();
        hasher.update(slice);
        let result = hasher.finalize().into();
        let val = U256::from_be_bytes(result);
        self.push(val);
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn environmental_info_opcode(
        &mut self,
        opcode: u8,
        _leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        match opcode {
            0x30 => {
                //ADDRESS
                let address = &execution_environment.i_address;
                let val = U256::from_be_bytes(address.into_word().0);
                self.push(val);
            }

            0x31 => {
                //BALANCE
                let val1 = self.pop();
                let address = Address::from_word(B256::from(val1.to_be_bytes::<32>()));
                let balance = state.get_balance(&address);
                match balance {
                    Some(x) => self.push(x),
                    None => self.push(U256::ZERO),
                }
                //SubStateの更新
                if !substate.a_access.contains(&address) {
                    substate.a_access.push(address.clone())
                }
            }

            0x32 => {
                //ORIGIN
                let address = &execution_environment.i_origin;
                let val = U256::from_be_bytes(address.into_word().0);
                self.push(val);
            }

            0x33 => {
                //CALLER
                let address = &execution_environment.i_sender;
                let val = U256::from_be_bytes(address.into_word().0);
                self.push(val);
            }

            0x34 => {
                //CALLVALUE
                let val = execution_environment.i_value;
                self.push(val);
            }

            0x36 => {
                //CALLDATASIZE
                let data = &execution_environment.i_data;
                self.push(U256::from(data.len()));
            }

            0x38 => {
                //CODESIZE
                let size = execution_environment.i_byte.len();
                self.push(U256::from(size));
            }

            0x3a => {
                //GASPRICE
                let price = execution_environment.i_gas_price;
                self.push(price);
            }

            0x3b => {
                //EXTCODESIZE
                let val1 = self.pop();
                let address = Address::from_word(B256::from(val1.to_be_bytes::<32>()));
                let result = state.get_code(&address);
                match result {
                    Some(x) => self.push(U256::from(x.len())),
                    None => self.push(U256::ZERO),
                }
                //SubStateの更新
                if !substate.a_access.contains(&address) {
                    substate.a_access.push(address.clone())
                }
            }

            0x3d => {
                //RETURNDATASIZE
                let size = self.return_back.len();
                self.push(U256::from(size));
            }

            0x3f => {
                //EXTCODEHASH
                let data = self.pop();
                let address = Address::from_word(B256::from(data.to_be_bytes::<32>()));
                //コード取得
                let result = state.get_code(&address);
                match result {
                    Some(x) => {
                        let mut hasher = Keccak256::new();
                        hasher.update(x);
                        let result = hasher.finalize().into();
                        let val = U256::from_be_bytes(result);
                        self.push(val);
                    }
                    None => self.push(U256::ZERO),
                }
                //SubStateの更新
                if !substate.a_access.contains(&address) {
                    substate.a_access.push(address.clone())
                }
            }
            _ => todo!(),
        }
    }

    #[inline(never)]
    fn calldataload_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //CALLDATALOAD
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let data = &execution_environment.i_data;
        let required_size = offset.saturating_add(32);
        let mut buffer = [0u8; 32];
        if offset >= data.len() {
            self.push(U256::ZERO);
            return;
        } else if required_size > data.len() {
            buffer[..data.len() - offset].copy_from_slice(&data[offset..data.len()]);
        } else {
            buffer[..].copy_from_slice(&data[offset..required_size]);
        }
        let val = U256::from_be_bytes(buffer);
        self.push(val);
    }

    #[inline(never)]
    fn calldatacopy_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //CALLDATACOPY
        let data = &execution_environment.i_data;
        let dest_offset = self.pop().try_into().unwrap_or(usize::MAX); //メモリ
        let offset = self.pop().try_into().unwrap_or(usize::MAX); //CALLDATA
        let size = self.pop().try_into().unwrap_or(usize::MAX);
        //メモリ拡張
        if size != 0 {
            let required_size = dest_offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = (required_size.saturating_add(31)) / 32;
                self.memory.resize(words * 32, 0);
            }
            //メモリに値を書き込む
            let mut slice = vec![0u8; size];
            let read_size = offset.saturating_add(size);
            if offset <= data.len() {
                if read_size > data.len() {
                    let copy_len = data.len() - offset;
                    slice[..copy_len].copy_from_slice(&data[offset..data.len()]);
                } else {
                    slice.copy_from_slice(&data[offset..read_size]);
                }
            }
            self.memory[dest_offset..required_size].copy_from_slice(&slice);
        }
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn codecopy_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //CODECOPY
        let data = &execution_environment.i_byte;
        let dest_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let size = self.pop().try_into().unwrap_or(usize::MAX);
        tracing::info!(
            dest_offset = dest_offset,
            offset = offset,
            size = size,
            "CODECOPY"
        );
        //メモリ拡張
        if size != 0 {
            let required_size = dest_offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = (required_size.saturating_add(31)) / 32;
                self.memory.resize(words * 32, 0);
            }
            //メモリに値を書き込む
            let mut slice = vec![0u8; size];
            let read_size = offset.saturating_add(size);
            if offset <= data.len() {
                if read_size > data.len() {
                    let copy_len = data.len() - offset;
                    slice[..copy_len].copy_from_slice(&data[offset..data.len()]);
                } else {
                    slice.copy_from_slice(&data[offset..read_size]);
                }
            }
            self.memory[dest_offset..required_size].copy_from_slice(&slice);
        }
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn extcodecopy_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        //EXTCODECOPY
        let val1 = self.pop();
        let address = Address::from_word(B256::from(val1.to_be_bytes::<32>()));
        let dest_offset = self.pop().try_into().unwrap_or(usize::MAX); //メモリ
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let size = self.pop().try_into().unwrap_or(usize::MAX);
        //コード取得
        let result = state.get_code(&address);
        let data = result.unwrap_or_default();
        //メモリ拡張
        if size != 0 {
            let required_size = dest_offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = (required_size.saturating_add(31)) / 32;
                self.memory.resize(words * 32, 0);
            }
            //メモリに値を書き込む
            let mut slice = vec![0u8; size];
            let read_size = offset.saturating_add(size);
            if offset <= data.len() {
                if read_size > data.len() {
                    let copy_len = data.len() - offset;
                    slice[..copy_len].copy_from_slice(&data[offset..data.len()]);
                } else {
                    slice.copy_from_slice(&data[offset..read_size]);
                }
            }
            self.memory[dest_offset..required_size].copy_from_slice(&slice);
        }
        //SubStateの更新
        if !substate.a_access.contains(&address) {
            substate.a_access.push(address.clone())
        }
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn returndatacopy_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        //RETURNDATACOPY
        let data = self.return_back.clone();
        let dest_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let size = self.pop().try_into().unwrap_or(usize::MAX);
        //メモリ拡張
        if size != 0 {
            let required_size = dest_offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = (required_size.saturating_add(31)) / 32;
                self.memory.resize(words * 32, 0);
            }
            //メモリに値を書き込む
            let read_size = offset.saturating_add(size);
            self.memory[dest_offset..required_size].copy_from_slice(&data[offset..read_size]);
        }
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn block_info_opcode(
        &mut self,
        opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        match opcode {
            0x40 => {
                //BLOCKHASH
                let header = &execution_environment.i_block_header;
                let num = self.pop();
                let my_num = header.h_number;
                if num > my_num.saturating_sub(U256::from(256)) {
                    //この場合はそのブロックのハッシュ値を返す
                } else {
                    self.push(U256::ZERO);
                }
            }

            0x41 => {
                //COINBASE
                let header = &execution_environment.i_block_header;
                let address = &header.h_beneficiary;
                let val = U256::from_be_bytes(address.into_word().0);
                self.push(val);
            }

            0x42 => {
                //TIMESTAMP
                let header = &execution_environment.i_block_header;
                let val = header.h_timestamp;
                self.push(val);
            }

            0x43 => {
                //NUMBER
                let header = &execution_environment.i_block_header;
                let my_num = header.h_number;
                self.push(my_num);
            }

            0x44 => {
                //PREVRANDAO
                let header = &execution_environment.i_block_header;
                let val = header.h_prevrandao;
                self.push(val);
            }

            0x45 => {
                //GASLIMIT
                let header = &execution_environment.i_block_header;
                let val = header.h_gaslimit;
                self.push(val);
            }

            0x46 => {
                //CHAINID 未実装
                self.push(U256::from(1));
            }

            0x47 => {
                //SELFBALANSE
                let address = &execution_environment.i_address;
                let balance = state.get_balance(address);
                match balance {
                    Some(x) => self.push(x),
                    None => self.push(U256::ZERO),
                }
            }

            0x48 => {
                //BASEFEE
                let header = &execution_environment.i_block_header;
                let val = header.h_basefee;
                self.push(val);
            }
            0_u8..=63_u8 | 73_u8..=u8::MAX => todo!(),
        }
    }

    #[inline(never)]
    fn mload_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        //MLOAD メモリから読み込む（32B)
        let pointer = self.pop().try_into().unwrap_or(usize::MAX);
        let required_size = pointer.saturating_add(32);
        //メモリ拡張
        if required_size > self.memory.len() {
            let words = required_size.saturating_add(31) / 32;
            self.memory.resize(words.saturating_mul(32), 0);
        }
        let slice = &self.memory[pointer..required_size];
        let mut tmp = [0u8; 32];
        tmp[..].copy_from_slice(slice);
        let val = U256::from_be_bytes(tmp);
        self.push(val);
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn mstore_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        //MSTORE メモリに保存(32)
        let pointer = self.pop().try_into().unwrap_or(usize::MAX);
        let data = self.pop();
        let required_size = pointer.saturating_add(32);
        //メモリ拡張
        if required_size > self.memory.len() {
            let words = required_size.saturating_add(31) / 32;
            self.memory.resize(words.saturating_mul(32), 0);
        }
        let slice = &mut self.memory[pointer..required_size];
        let bytes: [u8; 32] = data.to_be_bytes();
        slice.copy_from_slice(&bytes);
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn mstore8_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        _execution_environment: &ExecutionEnvironment,
    ) {
        //MSTORE8
        let pointer = self.pop().try_into().unwrap_or(usize::MAX);
        let data = self.pop();
        let required_size = pointer.saturating_add(1);
        //メモリ拡張
        if required_size > self.memory.len() {
            let words = required_size.saturating_add(31) / 32;
            self.memory.resize(words.saturating_mul(32), 0);
        }
        let slice = &mut self.memory[pointer..required_size];
        let bytes: [u8; 32] = data.to_be_bytes();
        slice.copy_from_slice(&bytes[31..32]);
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn sload_opcode(
        &mut self,
        _opcode: u8,
        _leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //SLOAD
        let key: U256 = self.pop();
        let address = &execution_environment.i_address;
        let value = state.get_storage_value(address, &key);
        //アクセス済みストレージキーリストの追加
        substate
            .a_access_storage
            .entry(address.clone())
            .or_default()
            .entry(key)
            .or_insert(value.unwrap_or(U256::ZERO));
        match value {
            Some(x) => self.push(x),
            None => self.push(U256::ZERO),
        }
    }

    #[inline(never)]
    fn sstore_opcode(
        &mut self,
        _opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //SSTORE
        let key = self.pop();
        let value = self.pop();
        let address = &execution_environment.i_address;
        //トランザクションが始まったときの一番最初の値を記録する
        let pre_value = state.get_storage_value(address, &key).unwrap_or(U256::ZERO);
        substate
            .a_access_storage
            .entry(address.clone())
            .or_default()
            .entry(key)
            .or_insert(pre_value);
        let _val0 = substate
            .a_access_storage
            .get(address)
            .unwrap()
            .get(&key)
            .cloned()
            .unwrap();

        //払い戻し
        if self.version < VersionId::London && self.version != VersionId::Constantinople {
            if !pre_value.is_zero() && value.is_zero() {
                substate.a_reimburse += 15000;
            }
        } else {
            let val0 = substate
                .a_access_storage
                .get(address)
                .unwrap()
                .get(&key)
                .cloned()
                .unwrap_or(U256::ZERO);
            if pre_value != value {
                if val0 == pre_value {
                    if val0 != U256::ZERO && value == U256::ZERO {
                        //0以外 →  0以外 → 0 :
                        if self.version == VersionId::Constantinople {
                            substate.a_reimburse += 15000;
                        } else {
                            substate.a_reimburse += 4800;
                        }
                    }
                } else {
                    if val0 != U256::ZERO && pre_value == U256::ZERO {
                        //0以外　→  0 →  0以外 : 返金の返金
                        if self.version == VersionId::Constantinople {
                            substate.a_reimburse -= 15000;
                        } else {
                            substate.a_reimburse -= 4800;
                        }
                    }
                    if val0 != U256::ZERO && value == U256::ZERO {
                        // 0以外(a) →  0以外(b) → 0 :返金
                        if self.version == VersionId::Constantinople {
                            substate.a_reimburse += 15000;
                        } else {
                            substate.a_reimburse += 4800;
                        }
                    }
                    if val0 == value {
                        if val0 == U256::ZERO {
                            //0 → 0以外 → 0
                            if self.version == VersionId::Constantinople {
                                substate.a_reimburse += 19800
                            } else {
                                substate.a_reimburse += 19900;
                            }
                        } else {
                            //0以外(a) → *(aではない)  →  0以外(a)
                            if self.version == VersionId::Constantinople {
                                substate.a_reimburse += 4800;
                            } else {
                                substate.a_reimburse += 2800;
                            }
                        }
                    }
                }
            }
        }
        //ステートを書き換える (0なら削除、それ以外なら保存)
        Action::Sstorage(address.clone(), key, U256::ZERO).push(leviathan, state);
        if value == U256::ZERO {
            state.remove_storage(address, key);
        } else {
            state.set_storage(address, key, value);
        }
    }

    #[inline(never)]
    fn push_opcode(
        &mut self,
        opcode: u8,
        _leviathan: &mut LEVIATHAN,
        _substate: &mut SubState,
        _state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        let code = &execution_environment.i_byte;
        let required_data_len = usize::from((opcode - 0x60) + 1);
        let mut buffer = [0u8; 32];

        //let data_number = code.len() - (self.pc + 1);
        let data_start = self.pc.saturating_sub(required_data_len);
        let data_end = self.pc;

        let copy_start = data_start.min(code.len());
        let copy_end = data_end.min(code.len());
        let actual_len = copy_end - copy_start;

        if actual_len > 0 {
            let buffer_start = 32 - required_data_len;
            buffer[buffer_start..buffer_start + actual_len]
                .copy_from_slice(&code[copy_start..copy_end]);
        }

        let data = U256::from_be_bytes(buffer);
        tracing::trace!("push {}", data);
        self.push(data);
    }

    #[inline(never)]
    fn log_opcode(
        &mut self,
        opcode: u8,
        _leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        _state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //LOG
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let size = self.pop().try_into().unwrap_or(usize::MAX);
        //topic
        let mut topic_n = opcode - 0xa0;
        let mut topic = Vec::new();
        while topic_n > 0 {
            let topi = self.pop();
            topic.push(topi);
            topic_n -= 1;
        }

        //メモリ読み取り
        let mut data = Vec::<u8>::new();
        if size > 0 {
            let required_size = offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = required_size.saturating_add(31) / 32;
                self.memory.resize(words.saturating_mul(32), 0);
            }
            let slice = &self.memory[offset..required_size];
            data = slice.to_vec();
        }
        //アドレス
        let address = &execution_environment.i_address;
        let log = Log::new(address.clone(), topic, data);
        substate.a_log.push(log);
        //アクティブなword数を更新
        let active_words = self.memory.len() / 32;
        self.active_words = active_words;
    }

    #[inline(never)]
    fn create_opcode(
        &mut self,
        _opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //CREATE
        let value = self.pop();
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let size = self.pop().try_into().unwrap_or(usize::MAX);
        //メモリ読み取り
        let mut data = Vec::<u8>::new();
        if size > 0 {
            let required_size = offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = required_size.saturating_add(31) / 32;
                self.memory.resize(words.saturating_mul(32), 0);
                let active_words = self.memory.len() / 32; //アクティブなword数を更新
                self.active_words = active_words;
            }
            let slice = &self.memory[offset..required_size];
            data = slice.to_vec();
        }
        //事前チェック
        //・残高チェック
        //・コールスタック深度
        //・Initコードサイズ
        let my_balance = state
            .get_balance(&execution_environment.i_address)
            .unwrap_or(U256::from(0));
        let is_balance = my_balance < value; //残高チェック
        let is_deepth = execution_environment.i_depth >= 1024;
        let is_code_size = if self.version >= VersionId::Shanghai {
            //Initcodeのサイズ確認
            data.len() > 49152
        } else {
            false
        };
        if is_balance || is_deepth || is_code_size {
            self.push(U256::ZERO);
            return;
        }
        //コントラクト自身のNonceのインクリメント
        Action::AddNonce(execution_environment.i_address.clone()).push(leviathan, state); //ロールバック用
        state.inc_nonce(&execution_environment.i_address);
        //depthのインクリメント
        let depth = execution_environment.i_depth + 1;
        //子に渡すガスの計算
        let mut child_gas = U256::from(0);
        if self.version < VersionId::TangerineWhistle {
            child_gas = self.gas
        } else {
            let gr = self.gas; //利用可能ガス
            child_gas = gr - (gr / U256::from(64)); //渡せる上限
        }
        self.gas = self.gas.saturating_sub(child_gas); //親からガスを徴収

        //Debug用
        let rem_stack1 = stacker::remaining_stack().unwrap_or(0);
        tracing::info!(
        value = %value,
        init_code = %hex::encode(&data),
        gas = %child_gas,
        rem_stack = rem_stack1,
        "CREATE",
        );

        //サブコールの実行
        let mut child_leviathan = Box::new(LEVIATHAN::new(self.version));
        let result = child_leviathan.contract_creation(
            state,
            substate,
            execution_environment.i_address.clone(),
            execution_environment.i_origin.clone(),
            child_gas,
            execution_environment.i_gas_price,
            value,
            data,
            depth,
            None,
            execution_environment.i_permission,
            execution_environment.i_block_header,
        );
        //実行後の処理
        match result {
            Ok((return_gas, _return_data, Some(contract_address))) => {
                //ガスの精算
                self.gas += return_gas;
                //return_backの更新
                self.return_back = Vec::<u8>::new();
                //新しいコントラクトアドレス
                let contract_u256 = U256::from_be_bytes(contract_address.into_word().0);
                //アクセス済みリストの更新
                if !substate.a_access.contains(&contract_address) {
                    substate.a_access.push(contract_address.clone())
                }
                tracing::info!("CREATE:0x{}", hex::encode(contract_address.0)); //アドレス
                //Journalのmerge
                leviathan.merge(*child_leviathan);
                //結果push
                self.push(contract_u256);
            }

            Err((return_gas, Some(return_data), _)) => {
                tracing::info!("CREATE: REVERT");
                //ガスの精算
                self.gas += return_gas;
                //return_backの更新
                self.return_back = return_data;
                self.push(U256::ZERO);
            }

            Err((_return_gas, None, _)) => {
                tracing::info!("CREATE: 例外停止");
                //ガスの精算
                self.push(U256::ZERO);
            }
            Ok((_, _, None)) => todo!(),
        }
    }

    #[inline(never)]
    fn create2_opcode(
        &mut self,
        _opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //CREATE2
        let value = self.pop();
        let offset = self.pop().try_into().unwrap_or(usize::MAX);
        let size = self.pop().try_into().unwrap_or(usize::MAX);
        let salt = self.pop();
        //メモリ読み取り
        let mut data = Vec::<u8>::new();
        if size > 0 {
            let required_size = offset.saturating_add(size);
            if required_size > self.memory.len() {
                let words = required_size.saturating_add(31) / 32;
                self.memory.resize(words.saturating_mul(32), 0);
                let active_words = self.memory.len() / 32; //アクティブなword数を更新
                self.active_words = active_words;
            }
            let slice = &self.memory[offset..required_size];
            data = slice.to_vec();
        }
        //事前チェック
        //・残高チェック
        //・コールスタック深度
        //・Initコードサイズ
        let my_balance = state
            .get_balance(&execution_environment.i_address)
            .unwrap_or(U256::from(0));
        let is_balance = my_balance < value; //残高チェック
        let is_deepth = execution_environment.i_depth >= 1024;
        let is_code_size = if self.version >= VersionId::Shanghai {
            //Initcodeのサイズ確認
            data.len() > 49152
        } else {
            false
        };
        if is_balance || is_deepth || is_code_size {
            self.push(U256::ZERO);
            return;
        }
        //コントラクト自身のNonceのインクリメント
        Action::AddNonce(execution_environment.i_address.clone()).push(leviathan, state); //ロールバック用
        state.inc_nonce(&execution_environment.i_address);
        //depthのインクリメント
        let depth = execution_environment.i_depth + 1;
        //子に渡すガスの計算
        let mut child_gas = U256::from(0);
        if self.version < VersionId::TangerineWhistle {
            child_gas = self.gas
        } else {
            let gr = self.gas; //利用可能ガス
            child_gas = gr - (gr / U256::from(64)); //渡せる上限
        }
        self.gas = self.gas.saturating_sub(child_gas); //親からガスを徴収
        //Debug用
        let rem_stack2 = stacker::remaining_stack().unwrap_or(0);
        tracing::info!(
        value = %value,
        init_code = %hex::encode(&data),
        gas = %child_gas,
        salt = %salt,
        rem_stack = rem_stack2,
        "CREATE2",
        );
        //サブコールの実行
        let mut child_leviathan = Box::new(LEVIATHAN::new(self.version));
        let result = child_leviathan.contract_creation(
            state,
            substate,
            execution_environment.i_address.clone(),
            execution_environment.i_origin.clone(),
            child_gas,
            execution_environment.i_gas_price,
            value,
            data,
            depth,
            Some(salt),
            execution_environment.i_permission,
            execution_environment.i_block_header,
        );
        //実行後の処理
        match result {
            Ok((return_gas, _return_data, Some(contract_address))) => {
                //ガスの精算
                self.gas += return_gas;
                //return_backの更新
                self.return_back = Vec::<u8>::new();
                //新しいコントラクトアドレス
                let contract_u256 = U256::from_be_bytes(contract_address.into_word().0);
                //アクセス済みリストの更新
                if !substate.a_access.contains(&contract_address) {
                    substate.a_access.push(contract_address.clone())
                }
                tracing::info!("CREATE2:0x{}", hex::encode(contract_address.0)); //アドレス
                //Journalのmerge
                leviathan.merge(*child_leviathan);
                //結果push
                self.push(contract_u256);
            }

            Err((return_gas, Some(return_data), _)) => {
                tracing::info!("CREATE2: Revert");
                //ガスの精算
                self.gas += return_gas;
                //return_backの更新
                self.return_back = return_data;
                self.push(U256::ZERO);
            }

            Err((_return_gas, None, _)) => {
                tracing::info!("CREATE2: 例外停止");
                //ガスの精算
                self.push(U256::ZERO);
            }
            Ok((_, _, None)) => todo!(),
        }
    }

    #[inline(never)]
    fn callcode_opcode(
        &mut self,
        _opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //CALLCODE
        let _gas = self.pop(); //サブコールに割り当てる最大ガス
        let to = self.pop(); //コードを借りてくる対象のアカウントアドレス
        let to_address = Address::from_word(B256::from(to.to_be_bytes::<32>()));
        let value = self.pop();
        let in_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let in_size = self.pop().try_into().unwrap_or(usize::MAX);
        let out_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let out_size = self.pop().try_into().unwrap_or(usize::MAX);
        //メモリ拡張
        let in_end = if in_size == 0 {
            0
        } else {
            in_offset.saturating_add(in_size)
        };

        let out_end = if out_size == 0 {
            0
        } else {
            out_offset.saturating_add(out_size)
        };
        let max_end = in_end.max(out_end);
        let mut data = Vec::<u8>::new();
        if max_end > self.memory.len() {
            let words = max_end.saturating_add(31) / 32;
            self.memory.resize(words.saturating_mul(32), 0);
            let active_words = self.memory.len() / 32; //アクティブなword数を更新
            self.active_words = active_words;
        }
        if in_size > 0 {
            let required_size = in_offset.saturating_add(in_size);
            let slice = &self.memory[in_offset..required_size];
            data = slice.to_vec();
        } else {
            data = Vec::<u8>::new();
        }
        //アクセス済みリストの更新
        if !substate.a_access.contains(&to_address) {
            substate.a_access.push(to_address.clone())
        }
        //子に渡すガスの計算
        let Some(mut child_gas) = self.child_gas_mem else {
            self.push(U256::ZERO);
            return;
        };
        self.child_gas_mem = None;
        if value > 0 {
            //最終的な子に渡すガス
            child_gas = child_gas.saturating_add(U256::from(2300)); //送金額が0よりも大きい
        } else {
            child_gas;
        }
        //デバック用
        tracing::info!(
        address =  format_args!("0x{}", hex::encode(to_address.0)),
        value = %value,
        data = %hex::encode(&data),
        gas = %child_gas,
        "CALLCODE",
        );
        //事前チェック
        //・残高チェック
        //・コールスタック深度
        let my_balance = state
            .get_balance(&execution_environment.i_address)
            .unwrap_or(U256::from(0));
        let is_balance = my_balance < value; //残高チェック
        let is_deepth = execution_environment.i_depth >= 1024;
        if is_balance || is_deepth {
            self.gas += child_gas;
            self.child_gas_mem = None;
            tracing::warn!("[CALLCODE] 事前チェックで例外停止");
            self.push(U256::ZERO);
            return;
        }
        //depthのインクリメント
        let depth = execution_environment.i_depth + 1;
        //サブコールの実行
        let mut child_leviathan = Box::new(LEVIATHAN::new(self.version));
        let result = child_leviathan.message_call(
            state,
            substate,
            execution_environment.i_address.clone(),
            execution_environment.i_origin.clone(),
            execution_environment.i_address.clone(),
            to_address.clone(),
            child_gas,
            execution_environment.i_gas_price,
            value,
            value,
            data,
            depth,
            execution_environment.i_permission,
            execution_environment.i_block_header,
        );
        //実行後の処理
        match result {
            Ok((return_gas, return_data, _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                tracing::info!(
                return_gas = %return_gas,
                "[CALLCODE] normal end:"
                );
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //Journalのmerge
                leviathan.merge(*child_leviathan);
                //結果push
                self.push(U256::from(1));
            }

            Err((return_gas, Some(return_data), _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //結果push
                self.push(U256::ZERO);
            }

            Err((_return_gas, None, _)) => {
                //結果push
                self.push(U256::ZERO);
            }
        }
    }

    #[inline(never)]
    fn delegatecall_opcode(
        &mut self,
        _opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //DELEGATECALL
        let _gas = self.pop(); //サブコールに割り当てる最大ガス
        let to = self.pop(); //呼び出し先のアドレス
        let to_address = Address::from_word(B256::from(to.to_be_bytes::<32>()));
        let in_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let in_size = self.pop().try_into().unwrap_or(usize::MAX);
        let out_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let out_size = self.pop().try_into().unwrap_or(usize::MAX);
        //メモリ拡張
        let in_end = if in_size == 0 {
            0
        } else {
            in_offset.saturating_add(in_size)
        };

        let out_end = if out_size == 0 {
            0
        } else {
            out_offset.saturating_add(out_size)
        };
        let max_end = in_end.max(out_end);
        let mut data = Vec::<u8>::new();
        if max_end > self.memory.len() {
            let words = max_end.saturating_add(31) / 32;
            self.memory.resize(words.saturating_mul(32), 0);
            let active_words = self.memory.len() / 32; //アクティブなword数を更新
            self.active_words = active_words;
        }
        if in_size > 0 {
            let required_size = in_offset.saturating_add(in_size);
            let slice = &self.memory[in_offset..required_size];
            data = slice.to_vec();
        } else {
            data = Vec::<u8>::new();
        }
        //子に渡すガスの計算
        let Some(child_gas) = self.child_gas_mem else {
            self.push(U256::ZERO);
            return;
        };
        //アクセス済みリストの更新
        if !substate.a_access.contains(&to_address) {
            substate.a_access.push(to_address.clone())
        }
        //デバック用
        tracing::info!(
        address =  format_args!("0x{}", hex::encode(to_address.0)),
        data = %hex::encode(&data),
        gas = %child_gas,
        "DELEGATECALL",
        );
        //事前チェック
        //・コールスタック深度
        let is_deepth = execution_environment.i_depth >= 1024;
        if is_deepth {
            self.gas += child_gas;
            self.child_gas_mem = None;
            tracing::warn!("[DELEGATECALL] 事前チェックで例外停止");
            self.push(U256::ZERO);
            return;
        }
        //depthのインクリメント
        let depth = execution_environment.i_depth + 1;
        //子に渡すガスの計算
        self.child_gas_mem = None;
        //サブコールの実行
        let mut child_leviathan = Box::new(LEVIATHAN::new(self.version));
        let result = child_leviathan.message_call(
            state,
            substate,
            execution_environment.i_sender.clone(),
            execution_environment.i_origin.clone(),
            execution_environment.i_address.clone(),
            to_address.clone(),
            child_gas,
            execution_environment.i_gas_price,
            U256::ZERO,
            execution_environment.i_value,
            data,
            depth,
            execution_environment.i_permission,
            execution_environment.i_block_header,
        );
        //実行後の処理
        match result {
            Ok((return_gas, return_data, _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //Journalのmerge
                leviathan.merge(*child_leviathan);
                //結果push
                self.push(U256::from(1));
            }

            Err((return_gas, Some(return_data), _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //結果push
                self.push(U256::ZERO);
            }

            Err((_return_gas, None, _)) => {
                //結果push
                self.push(U256::ZERO);
            }
        }
    }

    #[inline(never)]
    fn staticcall_opcode(
        &mut self,
        _opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) {
        //STATICCALL
        let _gas = self.pop(); //サブコールに割り当てる最大ガス
        let to = self.pop(); //呼び出し先のアドレス
        let to_address = Address::from_word(B256::from(to.to_be_bytes::<32>()));
        let in_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let in_size = self.pop().try_into().unwrap_or(usize::MAX);
        let out_offset = self.pop().try_into().unwrap_or(usize::MAX);
        let out_size = self.pop().try_into().unwrap_or(usize::MAX);
        //メモリ拡張
        let in_end = if in_size == 0 {
            0
        } else {
            in_offset.saturating_add(in_size)
        };

        let out_end = if out_size == 0 {
            0
        } else {
            out_offset.saturating_add(out_size)
        };
        let max_end = in_end.max(out_end);
        let mut data = Vec::<u8>::new();
        if max_end > self.memory.len() {
            let words = max_end.saturating_add(31) / 32;
            self.memory.resize(words.saturating_mul(32), 0);
            let active_words = self.memory.len() / 32; //アクティブなword数を更新
            self.active_words = active_words;
        }
        if in_size > 0 {
            let required_size = in_offset.saturating_add(in_size);
            let slice = &self.memory[in_offset..required_size];
            data = slice.to_vec();
        } else {
            data = Vec::<u8>::new();
        }
        //アクセス済みリストの更新
        if !substate.a_access.contains(&to_address) {
            substate.a_access.push(to_address.clone())
        }
        //子に渡すガスの計算
        let Some(child_gas) = self.child_gas_mem else {
            self.push(U256::ZERO);
            return;
        };
        //デバック用
        tracing::info!(
        address =  format_args!("0x{}", hex::encode(to_address.0)),
        data = %hex::encode(&data),
        gas = %child_gas,
        "STATICCALL",
        );
        //事前チェック
        //・残高チェック
        //・コールスタック深度
        let is_deepth = execution_environment.i_depth >= 1024;
        if is_deepth {
            self.gas += child_gas;
            self.child_gas_mem = None;
            tracing::warn!("[STATICCALL] 事前チェックで例外停止");
            self.push(U256::ZERO);
            return;
        }
        //depthのインクリメント
        let depth = execution_environment.i_depth + 1;
        //子に渡すガスの計算
        self.child_gas_mem = None;
        //サブコールの実行
        let mut child_leviathan = Box::new(LEVIATHAN::new(self.version));
        let result = child_leviathan.message_call(
            state,
            substate,
            execution_environment.i_address.clone(),
            execution_environment.i_origin.clone(),
            to_address.clone(),
            to_address.clone(),
            child_gas,
            execution_environment.i_gas_price,
            U256::ZERO,
            U256::ZERO,
            data,
            depth,
            false,
            execution_environment.i_block_header,
        );
        //実行後の処理
        match result {
            Ok((return_gas, return_data, _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //Journalのmerge
                leviathan.merge(*child_leviathan);
                //結果push
                self.push(U256::from(1));
            }

            Err((return_gas, Some(return_data), _)) => {
                //出力データのメモリ書き込み
                let return_size = return_data.len();
                let write_size = out_size.min(return_size); //書き込みサイズ
                if write_size > 0 {
                    let required_size = out_offset.saturating_add(write_size);
                    self.memory[out_offset..required_size]
                        .copy_from_slice(&return_data[..write_size]);
                }
                //Returndata バッファの更新
                self.return_back = return_data;
                //ガスの精算
                self.gas += return_gas;
                //結果push
                self.push(U256::ZERO);
            }

            Err((_return_gas, None, _)) => {
                //結果push
                self.push(U256::ZERO);
            }
        }
    }
}
