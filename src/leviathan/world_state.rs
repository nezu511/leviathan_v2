#![allow(dead_code)]

use alloy_primitives::{U256, B256};
use sha3::Digest;
use std::collections::HashMap;
use std::sync::Arc;
use eth_trie::{MemoryDB, EthTrie};
use alloy_rlp::{RlpEncodable, RlpDecodable};

pub struct WorldState(pub HashMap<Address, Account>);

pub struct WorldState2{
    cash: HashMap<Address, Account>,
    data: Arc<MemoryDB>,
    eth_trie: EthTrie<MemoryDB>,
    code_storage: HashMap<B256, Vec<u8>>
}

#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct MptAccount { //MPT専用
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: B256, 
    pub code_hash: B256,
}

#[derive(Debug, Eq, Hash, PartialEq, Clone, PartialOrd, Ord)]
pub struct Address(pub [u8; 20]);

impl Address {
    pub fn new(input: [u8; 20]) -> Self {
        Self(input)
    }

    pub fn from_u256(data: U256) -> Self {
        let bytes: [u8; 32] = data.to_be_bytes();
        let mut tmp = [0u8; 20];
        tmp.copy_from_slice(&bytes[12..32]);
        Self(tmp)
    }

    pub fn to_u256(&self) -> U256 {
        let mut tmp = [0u8; 32];
        tmp[12..32].copy_from_slice(&self.0);

        U256::from_be_bytes(tmp)
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub nonce: u32,
    pub balance: U256,
    pub storage: HashMap<U256, U256>,
    pub code: Vec<u8>,
}

impl Default for Account {
    fn default() -> Self {
        Self::new()
    }
}

impl Account {
    pub fn new() -> Self {
        Self {
            nonce: 0u32,
            balance: U256::ZERO,
            storage: HashMap::new(),
            code: Vec::new(),
        }
    }
}
