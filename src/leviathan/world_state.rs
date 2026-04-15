#![allow(dead_code)]

use alloy_primitives::{U256, B256, Address, keccak256, b256};
use sha3::Digest;
use std::collections::HashMap;
use std::sync::Arc;
use eth_trie::{MemoryDB, EthTrie, Trie};
use alloy_rlp::{RlpEncodable, RlpDecodable, Decodable};

// 空のMPTツリーのルートハッシュ (Keccak256(RLP("")))
pub const EMPTY_STORAGE_ROOT: B256 = b256!("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");
// 空のコードのハッシュ (Keccak256("")) 
pub const EMPTY_CODE_HASH: B256 = b256!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");



pub struct WorldState{
    pub cache: HashMap<Address, Account>,
    pub data: Arc<MemoryDB>,
    pub eth_trie: EthTrie<MemoryDB>,
    pub code_storage: HashMap<B256, Vec<u8>>
}

impl WorldState {
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

    pub fn add_cache(&mut self, address: &Address, mpt_accout: &MptAccount) {
        let nonce = mpt_accout.nonce;
        let balance = mpt_accout.balance;
        let shash = mpt_accout.storage_root;
        let code = self.code_storage.get(&mpt_accout.code_hash).cloned().unwrap();
        let account = Account::make(nonce, balance, code, shash);
        self.cache.insert(address.clone(), account);
    }

    pub fn contain_mpt(&mut self, address: &Address) -> Option<MptAccount> {
        //MPTを調査
        let address_hash = keccak256(address);
        let result = self.eth_trie.get(address_hash.as_slice()).unwrap();
        match result {
            Some(rlp_bytes) => {
                let mut slice = rlp_bytes.as_slice();
                let Ok(account) = MptAccount::decode(&mut slice) else {
                    tracing::warn!("[contain_mpt] MptAccount::decodeでエラー");
                    return None;
                };
                return Some(account);
            }

            None => return None,
        }
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
    pub fn new(state: &mut WorldState)  -> Self{
        //storage_root取得
        let mut storage_trie = EthTrie::new(state.data.clone());
        let storage_root = storage_trie.root_hash().unwrap();
        let code_hash = alloy_primitives::KECCAK256_EMPTY;
        Self {nonce: 0u64, balance: U256::ZERO, storage_root, code_hash}
    }
}


#[derive(Debug, Clone)]
pub struct Account {
    pub nonce: u64,
    pub balance: U256,
    pub storage: HashMap<U256, U256>,
    pub code: Vec<u8>,
    pub storage_hash: B256,
}

impl Default for Account {
    fn default() -> Self {
        Self::new()
    }
}

impl Account {
    pub fn new() -> Self {
        Self {
            nonce: 0u64,
            balance: U256::ZERO,
            storage: HashMap::new(),
            code: Vec::new(),
            storage_hash: EMPTY_STORAGE_ROOT,
        }
    }

    pub fn make(nonce: u64, balance: U256, code: Vec<u8>, shash: B256) -> Self {
        let storage = HashMap::<U256, U256>::new();
        Self {nonce, balance, storage, code, storage_hash:shash}
    }

}
