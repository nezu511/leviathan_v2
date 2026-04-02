#![allow(dead_code)]
/*
 *テストに対応するためBerlin以前の仕様に変更(called_cost 廃止(2100)!)
 *
*/
use crate::evm::evm::EVM;
use crate::leviathan::structs::{ExecutionEnvironment, SubState, VersionId};
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::evm_trait::{Gfunction, Xi};
use crate::my_trait::leviathan_trait::State;
use alloy_primitives::{I256, U256};

//GAS table固定費
static GAS_TABLE: [u8; 256] = {
    let mut table = [0; 256];

    // Stop and Arithmetic Operations
    table[0x00] = 0; // STOP
    table[0x01] = 3; // ADD
    table[0x02] = 5; // MUL
    table[0x03] = 3; // SUB
    table[0x04] = 5; // DIV
    table[0x05] = 5; // SDIV
    table[0x06] = 5; // MOD
    table[0x07] = 5; // SMOD
    table[0x08] = 8; // ADDMOD
    table[0x09] = 8; // MULMOD
    table[0x0a] = u8::MAX; // EXP
    table[0x0b] = 5; // SIGNEXTEND

    // Copmarison & Bitwise Logic Operations
    table[0x10] = 3; // LT
    table[0x11] = 3; // GT
    table[0x12] = 3; // SLT
    table[0x13] = 3; // SGT
    table[0x14] = 3; // EQ
    table[0x15] = 3; // ISZERO
    table[0x16] = 3; // ADN
    table[0x17] = 3; // OR
    table[0x18] = 3; // XOR
    table[0x19] = 3; // NOT
    table[0x1a] = 3; // BYTE
    table[0x1b] = 3; // SHL
    table[0x1c] = 3; // SHR
    table[0x1d] = 3; // SAR

    // Keccak256
    table[0x20] = u8::MAX; // KECCAK256

    // Environmental Information
    table[0x30] = 2; // ADDRESS
    table[0x31] = u8::MAX; // BALANCE
    table[0x32] = 2; // ORIGIN
    table[0x33] = 2; // CALLER
    table[0x34] = 2; // CALLVALUE
    table[0x35] = 3; // CALLDATALOAD
    table[0x36] = 2; // CALLDATASIZE
    table[0x37] = u8::MAX; // CALLDATACOPY
    table[0x38] = 2; // CODESIZE
    table[0x39] = u8::MAX; // CODECOPY
    table[0x3a] = 2; // GASPRICE
    table[0x3b] = u8::MAX; // EXTCODESIZE
    table[0x3c] = u8::MAX; // EXTCODECOPY
    table[0x3d] = 2; // RETURNDATASIZE
    table[0x3e] = u8::MAX; // RETURNDATACOPY
    table[0x3f] = u8::MAX; // EXTCODEHASH

    // Block Information
    table[0x40] = 20; // BLOCKHASH
    table[0x41] = 2; // COINBASE
    table[0x42] = 2; // TIMESTAMP
    table[0x43] = 2; // NUMER
    table[0x44] = 2; // PREVRANDAO
    table[0x45] = 2; // GASLIMIT
    table[0x46] = 2; // CHAINID
    table[0x47] = 5; // SELFBALANCE
    table[0x48] = 2; // BASEFEE

    // Stack, Memory, Storage and Flow Operations
    table[0x50] = 2; // POP
    table[0x51] = u8::MAX; // MLOAD
    table[0x52] = u8::MAX; // MSTORE
    table[0x53] = u8::MAX; // MSTORE8
    table[0x54] = u8::MAX; // SLOAD
    table[0x55] = u8::MAX; // SSTORE
    table[0x56] = 8; // JUMP
    table[0x57] = 10; // JUMPI
    table[0x58] = 2; // PC
    table[0x59] = 2; // MSIZE
    table[0x5a] = 2; // GAS
    table[0x5b] = 1; // JUMPDEST

    // push Operations
    table[0x5f] = 2; // PUSH0
    let mut i = 0x60;
    while i <= 0x7f {
        table[i] = 3;
        i += 1;
    }

    // Duplication Operations
    let mut i = 0x80;
    while i <= 0x8f {
        table[i] = 3;
        i += 1;
    }

    // Exchange Operations
    let mut i = 0x90;
    while i <= 0x9f {
        table[i] = 3;
        i += 1;
    }

    // Logging Operations
    let mut i = 0xa0;
    while i <= 0xa4 {
        table[i] = u8::MAX;
        i += 1;
    }

    // System operations
    let mut i = 0xf0;
    while i <= 0xff {
        table[i] = u8::MAX;
        i += 1;
    }

    table //ブロックの最後は原則返り値
};

impl Gfunction for EVM {
    fn extension_cost(&mut self, offset: U256, size: U256) -> U256 {
        if !size.is_zero() {
            let required_size = offset.saturating_add(size);
            let post_words = required_size.saturating_add(U256::from(31)) / U256::from(32);
            let pre_words = U256::from(self.active_words);
            if post_words > pre_words {
                let pre_cost =
                    (pre_words * U256::from(3)) + ((pre_words * pre_words) / U256::from(512));

                let post_sq = post_words.saturating_mul(post_words);
                let post_cost = post_words
                    .saturating_mul(U256::from(3))
                    .saturating_add(post_sq / U256::from(512));
                let result = post_cost.saturating_sub(pre_cost);
                return result;
            }
        }
        return U256::ZERO;
    }

    fn is_account_access(&mut self, data: U256, substate: &SubState) -> U256 {
        let buffer: [u8; 32] = data.to_be_bytes();
        let mut tmp = [0u8; 20];
        tmp.copy_from_slice(&buffer[12..32]);
        let address = Address::new(tmp);
        if substate.a_access.contains(&address) {
            return U256::from(100);
        } else {
            return U256::from(2600);
        }
    }

    fn gas(
        &mut self,
        opcode: u8,
        substate: &SubState,
        state: &WorldState,
        execution_environment: &ExecutionEnvironment,
    ) -> U256 {
        let used_gas = GAS_TABLE[opcode as usize];
        if used_gas != u8::MAX {
            return U256::from(used_gas);
        }

        let used_gas = match opcode {
            0x0a => {
                //EXP   OK対応!
                let exponent = self.peek(1);
                let bit = exponent.bit_len();
                let byte = (bit + 7) / 8;
                let byte_u256 = U256::from(byte);

                if self.version == VersionId::Frontier {
                    let result = byte_u256
                        .saturating_mul(U256::from(10))
                        .saturating_add(U256::from(10));
                    result
                } else {
                    let result = byte_u256
                        .saturating_mul(U256::from(50))
                        .saturating_add(U256::from(10));
                    result
                }
            }
            0x20 => {
                //KECCAK256
                //メモリ拡張コスト
                let offset = self.peek(0);
                let size = self.peek(1);
                let ext_cost = self.extension_cost(offset, size);
                //計算の動的コスト
                let words = size.saturating_add(U256::from(31)) / U256::from(32);
                let dynamic_cost = words.saturating_mul(U256::from(6));
                let total = ext_cost
                    .saturating_add(dynamic_cost)
                    .saturating_add(U256::from(30));
                return total;
            }

            0x31 | 0x3b => {
                //BALANCE
                //Address型に変換
                let data = self.peek(0);
                let cost = self.is_account_access(data, substate);
                return U256::from(cost);
            }

            0x3f => {
                //BALANCE
                //Address型に変換
                if self.version < VersionId::Istanbul {
                    return U256::from(400);
                } else if self.version < VersionId::Berlin {
                    return U256::from(700);
                } else {
                    let data = self.peek(0);
                    let cost = self.is_account_access(data, substate);
                    return U256::from(cost);
                }
            }

            0x37 | 0x39 | 0x3e => {
                //CALLDATACOPY, CODECOPY, RETURNDATACOPY
                let offset = self.peek(0);
                let size = self.peek(2);
                let ext_cost = self.extension_cost(offset, size);
                //計算の動的コスト
                let words = size.saturating_add(U256::from(31)) / U256::from(32);
                let dynamic_cost = words.saturating_mul(U256::from(3));
                let total = ext_cost
                    .saturating_add(dynamic_cost)
                    .saturating_add(U256::from(3));
                return total;
            }

            0x3c => {
                //EXTCODECOPY
                let address = self.peek(0);
                let offset = self.peek(1);
                let size = self.peek(3);
                //アドレスのアクセス状態
                let acc_cost = self.is_account_access(address, substate);
                //メモリの拡張コスト
                let ext_cost = self.extension_cost(offset, size);
                let words = size.saturating_add(U256::from(31)) / U256::from(32);
                let dynamic_cost = words.saturating_mul(U256::from(3));
                let total = acc_cost
                    .saturating_add(ext_cost)
                    .saturating_add(dynamic_cost);
                return total;
            }

            0x51 | 0x52 => {
                //MLOAD, MSTORE
                let offset = self.peek(0);
                let ext_cost = self.extension_cost(offset, U256::from(32));
                let total = ext_cost.saturating_add(U256::from(3));
                return total;
            }

            0x53 => {
                //MSTORE8
                let offset = self.peek(0);
                let ext_cost = self.extension_cost(offset, U256::from(1));
                let total = ext_cost.saturating_add(U256::from(3));
                return total;
            }

            0x54 => {
                //SLOAD
                let address = &execution_environment.i_address;
                let key = self.peek(0);
                if self.version < VersionId::TangerineWhistle {
                    U256::from(50)
                } else if self.version < VersionId::Istanbul {
                    U256::from(200)
                } else if self.version < VersionId::Berlin {
                    U256::from(800)
                } else {
                    let key_case = substate.a_access_storage.get(address);
                    if key_case.is_none() {
                        U256::from(2100)
                    } else {
                        if key_case.unwrap().contains_key(&key) {
                            U256::from(100)
                        } else {
                            U256::from(2100)
                        }
                    }
                }
            }

            0x55 => {
                //SSTORE
                let address = &execution_environment.i_address;
                let key = self.peek(0);
                let new_value = self.peek(1);
                //今現在，スロットに入ってる値
                let current_value = state
                    .get_storage_value(&address, &key)
                    .unwrap_or(U256::from(0));
                if self.version < VersionId::Istanbul && self.version != VersionId::Constantinople {
                    if current_value.is_zero() && !new_value.is_zero() {
                        U256::from(20000)
                    } else {
                        U256::from(5000)
                    }
                } else {
                    let mut called_cost = 0usize;
                    let key_case = substate.a_access_storage.get(address);
                    let original_value = if key_case.is_none() {
                        if self.version >= VersionId::Berlin {
                            called_cost = 2100; //Warm/Cold
                        }
                        current_value
                    } else {
                        let val1 = key_case.unwrap().get(&key);
                        if val1.is_none() {
                            if self.version >= VersionId::Berlin {
                                called_cost = 2100; //Warm/Cold
                            }
                            current_value
                        } else {
                            if self.version >= VersionId::Berlin {
                                called_cost = 100; //Warm/Cold
                            }
                            val1.unwrap().clone()
                        }
                    };
                    //Update Costを算出
                    let update_cost = if current_value == new_value {
                        //変更なし
                        if self.version == VersionId::Constantinople {
                            200
                        } else {
                            100
                        }
                    } else {
                        if current_value == original_value {
                            if original_value == U256::from(0) {
                                //0　→  0 →  0以外
                                20000
                            } else {
                                //0以外(a) →  0以外(a) →  0以外(b)
                                if self.version == VersionId::Constantinople {
                                    5000
                                } else {
                                    2900
                                }
                            }
                        } else {
                            //*(a) → *(b) →  *(c)
                            if self.version == VersionId::Constantinople {
                                200
                            } else {
                                100
                            }
                        }
                    };
                    //トータルcostを算出
                    let total = update_cost + called_cost;
                    return U256::from(total);
                }
            }

            0xa0..=0xa4 => {
                //LOG0 ~ LOG4
                let offset = self.peek(0);
                let size = self.peek(1);
                let ext_cost = self.extension_cost(offset, size);
                //topic cost
                let topic = opcode - 0xa0;
                let topic_cost = U256::from(topic).saturating_mul(U256::from(375));
                //dynamic_cost
                let dynamic_cost = size.saturating_mul(U256::from(8));
                let total = ext_cost
                    .saturating_add(topic_cost)
                    .saturating_add(dynamic_cost)
                    .saturating_add(U256::from(375));
                return total;
            }

            0xf0 => {
                //CREATE
                let offset = self.peek(1);
                let size = self.peek(2);
                //拡張コスト
                let ext_cost = self.extension_cost(offset, size);
                let words = size.saturating_add(U256::from(31)) / U256::from(32);
                let dynamic_cost = words.saturating_mul(U256::from(2));
                if self.version < VersionId::Shanghai {
                    let total = ext_cost.saturating_add(U256::from(32000));
                    return total;
                } else {
                    let total = dynamic_cost
                        .saturating_add(ext_cost)
                        .saturating_add(U256::from(32000));
                    return total;
                }
            }

            0xf1 => {
                //CALL
                let child_gas_limit = self.peek(0);
                let address = self.peek(1);
                let value = self.peek(2);
                let args_offset = self.peek(3);
                let args_size = self.peek(4);
                let ret_offset = self.peek(5);
                let ret_size = self.peek(6);
                //メモリ拡張コスト
                let args_end = if args_size.is_zero() {
                    U256::ZERO
                } else {
                    args_offset.saturating_add(args_size)
                };

                let ret_end = if ret_size.is_zero() {
                    U256::ZERO
                } else {
                    ret_offset.saturating_add(ret_size)
                };
                let max_end = args_end.max(ret_end);
                let ext_cost = self.extension_cost(U256::ZERO, max_end);
                //アドレスのアクセス状態
                let acc_cost = if self.version < VersionId::TangerineWhistle {
                    U256::from(40)
                } else if self.version < VersionId::Berlin {
                    U256::from(700)
                } else {
                    self.is_account_access(address, substate)
                };
                //送金とアカウント作成の追加コスト
                let address = Address::from_u256(address);
                let mut create_cost = U256::ZERO;
                if self.version < VersionId::SpuriousDragon {
                    if !value.is_zero() {
                        create_cost = create_cost.saturating_add(U256::from(9000));
                    }
                    if state.is_empty(&address) {
                        create_cost = create_cost.saturating_add(U256::from(25000));
                    }
                } else {
                    if !value.is_zero() {
                        create_cost = create_cost.saturating_add(U256::from(9000));
                        if state.is_empty(&address) {
                            create_cost = create_cost.saturating_add(U256::from(25000));
                        }
                    }
                }
                let base_cost = ext_cost
                    .saturating_add(acc_cost)
                    .saturating_add(create_cost);
                //子に渡すガス
                let mut result = U256::ZERO;
                if self.version < VersionId::TangerineWhistle {
                    result = child_gas_limit;
                } else {
                    let gr = self.gas.saturating_sub(base_cost);
                    let gr = gr - (gr / U256::from(64));
                    if gr > child_gas_limit {
                        result = child_gas_limit;
                    } else {
                        result = gr;
                    }
                }
                self.child_gas_mem = Some(result);
                return result.saturating_add(base_cost);
            }

            0xf2 => {
                //CALLCODE
                let child_gas_limit = self.peek(0);
                let address = self.peek(1);
                let value = self.peek(2);
                let args_offset = self.peek(3);
                let args_size = self.peek(4);
                let ret_offset = self.peek(5);
                let ret_size = self.peek(6);
                //メモリ拡張コスト
                let args_end = if args_size.is_zero() {
                    U256::ZERO
                } else {
                    args_offset.saturating_add(args_size)
                };

                let ret_end = if ret_size.is_zero() {
                    U256::ZERO
                } else {
                    ret_offset.saturating_add(ret_size)
                };
                let max_end = args_end.max(ret_end);
                let ext_cost = self.extension_cost(U256::ZERO, max_end);
                //アドレスのアクセス状態
                let acc_cost = if self.version < VersionId::TangerineWhistle {
                    U256::from(40)
                } else if self.version < VersionId::Berlin {
                    U256::from(700)
                } else {
                    self.is_account_access(address, substate)
                };
                //送金とアカウント作成の追加コスト
                let address = Address::from_u256(address);
                let mut create_cost = U256::from(0);
                if !value.is_zero() {
                    create_cost = create_cost.saturating_add(U256::from(9000));
                }
                let base_cost = ext_cost
                    .saturating_add(acc_cost)
                    .saturating_add(create_cost);
                //子に渡すガス
                let mut result = U256::ZERO;
                if self.version < VersionId::TangerineWhistle {
                    result = child_gas_limit;
                } else {
                    let gr = self.gas.saturating_sub(base_cost);
                    let gr = gr - (gr / U256::from(64));
                    if gr > child_gas_limit {
                        result = child_gas_limit;
                    } else {
                        result = gr;
                    }
                }
                self.child_gas_mem = Some(result);
                return result.saturating_add(base_cost);
            }

            0xf3 | 0xfd => {
                //RETURN
                let offset = self.peek(0);
                let size = self.peek(1);
                let ext_cost = self.extension_cost(offset, size);
                return ext_cost;
            }

            0xf4 | 0xfa => {
                //DELEGATECALL, STATICCALL
                let child_gas_limit = self.peek(0);
                let address = self.peek(1);
                let args_offset = self.peek(2);
                let args_size = self.peek(3);
                let ret_offset = self.peek(4);
                let ret_size = self.peek(5);
                //メモリ拡張コスト
                let args_end = if args_size.is_zero() {
                    U256::ZERO
                } else {
                    args_offset.saturating_add(args_size)
                };

                let ret_end = if ret_size.is_zero() {
                    U256::ZERO
                } else {
                    ret_offset.saturating_add(ret_size)
                };
                let max_end = args_end.max(ret_end);
                let ext_cost = self.extension_cost(U256::ZERO, max_end);
                //アドレスのアクセス状態
                let acc_cost = if self.version < VersionId::TangerineWhistle {
                    U256::from(40)
                } else if self.version < VersionId::Berlin {
                    U256::from(700)
                } else {
                    self.is_account_access(address, substate)
                };

                let base_cost = ext_cost.saturating_add(acc_cost);
                //子に渡すガス
                let mut result = U256::ZERO;
                if self.version < VersionId::TangerineWhistle {
                    result = child_gas_limit;
                } else {
                    let gr = self.gas.saturating_sub(base_cost);
                    let gr = gr - (gr / U256::from(64));
                    if gr > child_gas_limit {
                        result = child_gas_limit;
                    } else {
                        result = gr;
                    }
                }
                self.child_gas_mem = Some(result);
                return result.saturating_add(base_cost);
            }

            0xf5 => {
                //CREATE2
                let offset = self.peek(1);
                let size = self.peek(2);
                //拡張コスト
                let ext_cost = self.extension_cost(offset, size);
                let words = size.saturating_add(U256::from(31)) / U256::from(32);
                let dynamic_cost = words.saturating_mul(U256::from(8));
                let total = dynamic_cost
                    .saturating_add(ext_cost)
                    .saturating_add(U256::from(32000));
                return total;
            }

            0xff => {
                if self.version < VersionId::TangerineWhistle {
                    return U256::ZERO;
                } else {
                    let data = self.peek(0);
                    let address = Address::from_u256(data);
                    //新規アカウント作成のペナルティ
                    let my_address = &execution_environment.i_address;
                    let create_cost = if state.get_balance(my_address).unwrap_or(U256::from(0))
                        > U256::from(0)
                        && state.is_empty(&address)
                    {
                        25000
                    } else {
                        0
                    };
                    let access_state_cost = 0usize;
                    if self.version > VersionId::Berlin {
                        //送り先のアドレスのアクセス状態
                        if substate.a_access.contains(&address) {
                            0usize
                        } else {
                            2600
                        };
                    }
                    let total = create_cost + access_state_cost + 5000;
                    return U256::from(total);
                }
            }

            _ => U256::from(0),
        };
        return used_gas;
    }
}
