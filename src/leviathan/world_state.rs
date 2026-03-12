#![allow(dead_code)]

use primitive_types::U256;
use std::collections::HashMap;
use sha3::{Keccak256, Digest};

pub struct WorldState(pub HashMap<Address, Account>);

pub struct Address(pub [u8;20]);

pub struct Account {
    pub nonce: u32,
    pub balance: U256, 
    pub storage: HashMap<U256, U256>,
    pub code: Vec<u8>,
}

