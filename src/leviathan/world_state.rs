#![allow(dead_code)]

use alloy_primitives::{U256, B256, Address, keccak256};
use sha3::Digest;
use std::collections::HashMap;
use std::sync::Arc;
use eth_trie::{MemoryDB, EthTrie, Trie};
use alloy_rlp::{RlpEncodable, RlpDecodable};

pub struct WorldState(pub HashMap<Address, Account>);

pub struct WorldState2{
    cache: HashMap<Address, Account>,
    data: Arc<MemoryDB>,
    eth_trie: EthTrie<MemoryDB>,
    code_storage: HashMap<B256, Vec<u8>>
}

impl WorldState2 {
    pub fn new() -> Self {
        let data = Arc::new(MemoryDB::new(true));
        let cache = HashMap::<Address, Account>::new();
        let eth_trie = EthTrie::new(data.clone());
        let mut code_storage = HashMap::<B256, Vec<u8>>::new();
        //空のコードのハッシュを登録
        let empty_code = Vec::<u8>::new();
        let hash = keccak256(&empty_code);
        code_storage.insert(hash, empty_code);

        Self {cache, data, eth_trie, code_storage}
    }
}
        


#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct MptAccount { //MPT専用
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: B256, 
    pub code_hash: B256,
}

impl MptAccount {
    pub fn new(state: &mut WorldState2)  -> Self{
        //storage_root取得
        let mut storage_trie = EthTrie::new(state.data.clone());
        let storage_root = storage_trie.root_hash().unwrap();
        let code_hash = alloy_primitives::KECCAK256_EMPTY;
        Self {nonce: 0u64, balance: U256::ZERO, storage_root, code_hash}
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
