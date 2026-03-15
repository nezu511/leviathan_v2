#![allow(dead_code)]

use primitive_types::U256;
use std::collections::HashMap;
use sha3::{Keccak256, Digest};

pub struct WorldState(pub HashMap<Address, Account>);

impl WorldState {
    pub fn is_empty(&self,address: &Address) -> bool {  //空だとtrue
        if self.0.contains_key(address) {
            let account = self.0.get(address).unwrap();
            if account.nonce != 0 ||  !account.balance.is_zero() || account.code.len() != 0 {
                return false;
            }
        }
        return true;
    }
}
            


#[derive(Debug,Eq, Hash, PartialEq,Clone)]
pub struct Address(pub [u8;20]);

impl Address {
    pub fn new(input: [u8;20]) -> Self {
        Self (input)
    }

    pub fn from_u256(data:U256) -> Self{
        let buffer = &data.to_big_endian()[12..32];
        let mut tmp = [0u8;20];
        tmp[0..20].copy_from_slice(&buffer[0..20]);
        Self (tmp)
    }

}

pub struct Account {
    pub nonce: u32,
    pub balance: U256, 
    pub storage: HashMap<U256, U256>,
    pub code: Vec<u8>,
}

