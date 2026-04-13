#![allow(dead_code)]

use alloy_primitives::{U256, B256, Address};
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

impl WorldState2 {
    //data:Arc<MemoryDB> = Arc::new(MemoryDB::new(true))
    pub fn new(data: Arc<MemoryDB>) -> Self {
        let cash = HashMap::<Address, Account>::new();
        let data = data;
        let eth_trie = EthTrie::new(data.clone());
        let code_storage = HashMap::<B256, Vec<u8>>::new();

        Self {cash, data, eth_trie, code_storage}
    }
}
        


#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct MptAccount { //MPT専用
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: B256, 
    pub code_hash: B256,
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
