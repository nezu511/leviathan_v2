#![allow(dead_code)]

use primitive_types::U256;
use std::collections::HashMap;
use sha3::{Keccak256, Digest};

pub struct WorldState(pub HashMap<Address, Account>);


#[derive(Debug,Eq, Hash, PartialEq,Clone)]
pub struct Address(pub [u8;20]);

impl Address {
    pub fn new(input: [u8;20]) -> Self {
        Self (input)
    }
}

pub struct Account {
    pub nonce: u32,
    pub balance: U256, 
    pub storage: HashMap<U256, U256>,
    pub code: Vec<u8>,
}

