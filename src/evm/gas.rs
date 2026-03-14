#![allow(dead_code)]

use primitive_types::U256; 
use crate::my_trait::evm_trait::{Xi, Gfunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use crate::evm::evm::EVM;

//GAS table固定費
static GAS_TABLE: [u8; 256] = {
    let mut table = [0; 256];
    
    // Stop and Arithmetic Operations
    table[0x00] = 0;        // STOP
    table[0x01] = 3;        // ADD
    table[0x02] = 5;        // MUL
    table[0x03] = 3;        // SUB
    table[0x04] = 5;        // DIV
    table[0x05] = 5;        // SDIV
    table[0x06] = 5;        // MOD
    table[0x07] = 5;        // SMOD
    table[0x08] = 8;        // ADDMOD
    table[0x09] = 8;        // MULMOD
    table[0x0a] = u8::MAX;  // EXP
    table[0x0b] = 5;        // SIGNEXTEND

    // Copmarison & Bitwise Logic Operations
    table[0x10] = 3;        // LT
    table[0x11] = 3;        // GT
    table[0x12] = 3;        // SLT
    table[0x13] = 3;        // SGT
    table[0x14] = 3;        // EQ
    table[0x15] = 3;        // ISZERO
    table[0x16] = 3;        // ADN
    table[0x17] = 3;        // OR
    table[0x18] = 3;        // XOR
    table[0x19] = 3;        // NOT
    table[0x1a] = 3;        // BYTE
    table[0x1b] = 3;        // SHL
    table[0x1c] = 3;        // SHR
    table[0x1d] = 3;        // SAR

    // Keccak256
    table[0x20] = u8::MAX;  // KECCAK256

    // Environmental Information
    table[0x30] = 2;        // ADDRESS
    table[0x31] = u8::MAX;  // BALANCE
    table[0x32] = 2;        // ORIGIN
    table[0x33] = 2;        // CALLER
    table[0x34] = 2;        // CALLVALUE
    table[0x35] = 3;        // CALLDATALOAD
    table[0x36] = 2;        // CALLDATASIZE
    table[0x37] = u8::MAX;  // CALLDATACOPY
    table[0x38] = 2;        // CODESIZE
    table[0x39] = u8::MAX;  // CODECOPY
    table[0x3a] = 2;        // GASPRICE
    table[0x3b] = u8::MAX;  // EXTCODESIZE
    table[0x3c] = u8::MAX;  // EXTCODECOPY
    table[0x3d] = 2;        // RETURNDATASIZE
    table[0x3e] = u8::MAX;  // RETURNDATACOPY
    table[0x3f] = u8::MAX;  // EXTCODEHASH
                            
    // Block Information
    table[0x40] = 20;        // BLOCKHASH
    table[0x41] = 20;        // COINBASE
    table[0x42] = 20;        // TIMESTAMP
    table[0x43] = 20;        // NUMER
    table[0x44] = 20;        // PREVRANDAO
    table[0x45] = 20;        // GASLIMIT
    table[0x46] = 20;        // CHAINID
    table[0x47] = 20;        // SELFBALANCE
    table[0x48] = 20;        // BASEFEE

    // Stack, Memory, Storage and Flow Operations
    table[0x51] = u8::MAX;   // MLOAD
    table[0x52] = u8::MAX;   // MSTORE
    table[0x53] = u8::MAX;   // MSTORE8
    table[0x54] = u8::MAX;   // SLOAD
    table[0x55] = u8::MAX;   // SSTORE
    table[0x56] = 8;         // JUMP
    table[0x57] = 10;        // JUMPI
    table[0x58] = 2;         // PC
    table[0x59] = 2;         // MSIZE
    table[0x5a] = 2;         // GAS
    table[0x5b] = 1;         // JUMPDEST

    // push Operations
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

    table   //ブロックの最後は原則返り値
};



impl Gfunction for EVM {
    fn gas(&mut self, opcode:u8, execution_environment: ExecutionEnvironment) -> U256 {
        todo!()
    }
}
