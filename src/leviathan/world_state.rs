#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use std::collections::HashMap;
use sha3::{Keccak256, Digest};

pub struct WorldState(pub HashMap<Address, Account>);



#[derive(Debug,Eq, Hash, PartialEq,Clone)]
pub struct Address(pub [u8;20]);

impl Address {
    pub fn new(input: [u8;20]) -> Self {
        Self (input)
    }

    pub fn from_u256(data:U256) -> Self{
        let bytes:[u8;32] = data.to_be_bytes();
        let mut tmp = [0u8;20];
        tmp.copy_from_slice(&bytes[12..32]);
        Self (tmp)
    }

    pub fn to_u256(&self) -> U256 {
        let mut tmp = [0u8; 32];
        tmp[12..32].copy_from_slice(&self.0);
        let val = U256::from_be_bytes(tmp);
        return val
    }

}

pub struct Account {
    pub nonce: u32,
    pub balance: U256, 
    pub storage: HashMap<U256, U256>,
    pub code: Vec<u8>,
}


impl Account {
    pub fn new() -> Self {
        Self { nonce:0u32, balance:U256::ZERO, storage:HashMap::new(), code:Vec::new()}
    }
}

