#![allow(dead_code)]

use primitive_types::U256; 
use crate::my_trait::evm_trait::{Xi, Gfunction, Zfunction};
use crate::my_trait::leviathan_trait::State;
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use crate::evm::evm::EVM;

//push_pop表
static SAFE_TABLE: [[u8;2];256] = {
    //[pop, push]
    let mut table = [[u8::MAX;2];256];

    // Stop and Arithmetic Operations
    table[0x00] = [0,0];        // STOP
    table[0x01] = [2,1];        // ADD
    table[0x02] = [2,1];        // MUL
    table[0x03] = [2,1];        // SUB
    table[0x04] = [2,1];        // DIV
    table[0x05] = [2,1];        // SDIV
    table[0x06] = [2,1];        // MOD
    table[0x07] = [2,1];        // SMOD
    table[0x08] = [3,1];        // ADDMOD
    table[0x09] = [3,1];        // MULMOD
    table[0x0a] = [2,1];  // EXP
    table[0x0b] = [2,1];        // SIGNEXTEND

    // Copmarison & Bitwise Logic Operations
    table[0x10] = [2,1];        // LT
    table[0x11] = [2,1];        // GT
    table[0x12] = [2,1];        // SLT
    table[0x13] = [2,1];        // SGT
    table[0x14] = [2,1];        // EQ
    table[0x15] = [1,1];        // ISZERO
    table[0x16] = [2,1];        // ADN
    table[0x17] = [2,1];        // OR
    table[0x18] = [2,1];        // XOR
    table[0x19] = [1,1];        // NOT
    table[0x1a] = [2,1];        // BYTE
    table[0x1b] = [2,1];        // SHL
    table[0x1c] = [2,1];        // SHR
    table[0x1d] = [2,1];        // SAR

    // Keccak256
    table[0x20] = [2,1];  // KECCAK256

    // Environmental Information
    table[0x30] = [0,1];        // ADDRESS
    table[0x31] = [2,1];  // BALANCE
    table[0x32] = [0,1];        // ORIGIN
    table[0x33] = [0,1];        // CALLER
    table[0x34] = [0,1];        // CALLVALUE
    table[0x35] = [1,1];        // CALLDATALOAD
    table[0x36] = [0,1];        // CALLDATASIZE
    table[0x37] = [3,0];  // CALLDATACOPY
    table[0x38] = [0,1];        // CODESIZE
    table[0x39] = [3,0];  // CODECOPY
    table[0x3a] = [0,1];        // GASPRICE
    table[0x3b] = [1,1];  // EXTCODESIZE
    table[0x3c] = [4,0];  // EXTCODECOPY
    table[0x3d] = [0,1];        // RETURNDATASIZE
    table[0x3e] = [3,0];  // RETURNDATACOPY
    table[0x3f] = [1,1];  // EXTCODEHASH
                            
    // Block Information
    table[0x40] = [1,1];        // BLOCKHASH
    table[0x41] = [0,1];        // COINBASE
    table[0x42] = [0,1];        // TIMESTAMP
    table[0x43] = [0,1];        // NUMER
    table[0x44] = [0,1];        // PREVRANDAO
    table[0x45] = [0,1];        // GASLIMIT
    table[0x46] = [0,1];        // CHAINID
    table[0x47] = [0,1];        // SELFBALANCE
    table[0x48] = [0,1];        // BASEFEE

    // Stack, Memory, Storage and Flow Operations
    table[0x50] = [1,0];   // POP
    table[0x51] = [1,1];   // MLOAD
    table[0x52] = [2,0];   // MSTORE
    table[0x53] = [2,0];   // MSTORE8
    table[0x54] = [1,1];   // SLOAD
    table[0x55] = [2,0];   // SSTORE
    table[0x56] = [1,0];         // JUMP
    table[0x57] = [2,0];        // JUMPI
    table[0x58] = [0,1];         // PC
    table[0x59] = [0,1];         // MSIZE
    table[0x5a] = [0,1];         // GAS
    table[0x5b] = [0,0];         // JUMPDEST

    // push Operations
    let mut i = 0x5f;
    while i <= 0x7f {
        table[i] = [0,1];
        i += 1;
    }

    // Duplication Operations
    let mut a = 1;
    let mut b = 2;
    let mut i = 0x80;
    while i <= 0x8f {
        table[i] = [a,b];
        i += 1;
        a += 1;
        b += 1;
    }

    // Exchange Operations
    let mut i = 0x90;
    let mut a = 2;
    while i <= 0x9f {
        table[i] = [a,a];
        i += 1;
        a += 1;
    }

    // Logging Operations
    let mut i = 0xa0;
    let mut a = 2;
    while i <= 0xa4 {
        table[i] = [a,0];
        i += 1;
        a += 1;
    }

    //System Operations
    table[0xf0] = [3,1];         // CREATE
    table[0xf1] = [7,1];         // CALL
    table[0xf2] = [7,1];         // CALLCODE
    table[0xf3] = [2,0];         // RETURN
    table[0xf4] = [6,1];         // DELEGATECALL
    table[0xf5] = [4,1];         // CREATE2
    table[0xfa] = [6,1];         // STATICCALL
    table[0xfd] = [2,0];         // REVERT
    table[0xff] = [1,0];         // SELFDESTRUCT

    table
};


impl Zfunction for EVM {

    fn is_safe(&mut self, opcode:u8, substate: &SubState, state: &WorldState, execution_environment: ExecutionEnvironment) -> bool  {
        //命令のガスコストと現在の残ガスを比較
        let gas_cost = self.gas(opcode, substate, state, &execution_environment);
        if  self.gas < gas_cost {
            return false;
        }

        //不正な命令の実行確認
        //SAFE_TABLEの値がu8::MAXは不正
        let op_info = SAFE_TABLE[opcode as usize];
        if op_info[0] == u8::MAX {
            return false;
        }

        //現在の命令が要求する要素数に対して，スタックの中身は足りるか？
        let pop_number = op_info[0];
        if self.stack.len() < pop_number as usize {
            return false;
        }

        //現在の命令を実行すると，スタックサイズが1024を超える
        let push_number = op_info[1];
        let stack_size = self.stack.len() + push_number as usize;
        if stack_size > 1024 {
            return false;
        }

        //スタックが指定する飛び先の位置が有効か
        if opcode == 0x56 || opcode == 0x57 {
            let distination = self.stack[0].as_usize();
            if self.safe_jump[distination] != 1 {
                return false;
            }
        }

        //命令がSSTOREで残ガスが2300以下
        if opcode == 0x55 && self.gas <= U256::from(2300) {
            return false;
        }

        //RETURNDATACOPYに関するルール
        if opcode == 0x3c {
            let offset = self.stack[1].as_usize();
            let size = self.stack[2].as_usize();
            let required_size = offset + size;
            if required_size > self.return_back.len() {
                return false;
            }
        }

        return true;
    }
}

