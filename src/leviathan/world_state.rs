#![allow(dead_code)]

use alloy_primitives::{Address, B256, U256, b256, keccak256, hex};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use eth_trie::{EthTrie, MemoryDB, Trie};
use sha3::Digest;
use std::collections::HashMap;
use std::sync::Arc;

// 空のMPTツリーのルートハッシュ (Keccak256(RLP("")))
pub const EMPTY_STORAGE_ROOT: B256 =
    b256!("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");
// 空のコードのハッシュ (Keccak256(""))
pub const EMPTY_CODE_HASH: B256 =
    b256!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");

pub struct WorldState {
    pub cache: HashMap<Address, Account>,
    pub data: Arc<MemoryDB>,
    pub eth_trie: EthTrie<MemoryDB>,
    pub code_storage: HashMap<B256, Vec<u8>>,
}

impl WorldState {
    pub fn new() -> Self {
        let data = Arc::new(MemoryDB::new(true));
        let cache = HashMap::<Address, Account>::new();
        let mut eth_trie = EthTrie::new(data.clone());
        let _ = eth_trie.root_hash().unwrap();
        let mut code_storage = HashMap::<B256, Vec<u8>>::new();
        //空のコードのハッシュを登録
        let empty_code = Vec::<u8>::new();
        let hash = keccak256(&empty_code);
        code_storage.insert(hash, empty_code);

        Self {
            cache,
            data,
            eth_trie,
            code_storage,
        }
    }

    pub fn init_mpt_account(&mut self, address: &Address, cache_account: &Account) {
        let mpt_nonce = cache_account.nonce;
        let mpt_balance = cache_account.balance;
        let mut storage_trie = EthTrie::from(self.data.clone(), cache_account.storage_hash).unwrap();
        let mut storage_changed = false;
        //storageの値を書き込む
        for (key, value) in cache_account.storage.iter() {
            let key_byte: [u8; 32] = key.to_be_bytes();
            let key_hash = keccak256(key_byte);
            let existing_val_opt =
                storage_trie.get(key_hash.as_slice()).unwrap_or(None);

            if value.is_zero() {
                if existing_val_opt.is_some() {
                    storage_trie.remove(key_hash.as_slice());
                    storage_changed = true;
                }
            } else {
                let val_rlp_bytes = alloy_rlp::encode(value);
                if existing_val_opt != Some(val_rlp_bytes.clone()) {
                    storage_trie
                        .insert(key_hash.as_slice(), val_rlp_bytes.as_slice())
                        .unwrap();
                    storage_changed = true;
                }
            }
        }
        //新しいstorage_rootを取得
        let storage_root = if storage_changed {
            storage_trie.root_hash().unwrap()
        } else {
            cache_account.storage_hash
        };
        //コードハッシュを取得
        let code_hash = keccak256(cache_account.code.clone());
        self
            .code_storage
            .entry(code_hash)
            .or_insert(cache_account.code.clone());
        //mpt_accout作成
        let mpt_account = MptAccount::new(
            cache_account.nonce,
            cache_account.balance,
            storage_root,
            code_hash,
            );
        //MPTに書き込む
        let address_hash = keccak256(address);
        let mut mpt_account_rlp_bytes = Vec::new();
        mpt_account.encode(&mut mpt_account_rlp_bytes);

        //MPTに現在登録されているRLPを取得
        let existing_mpt_val =
            self.eth_trie.get(address_hash.as_slice()).unwrap_or(None);

        // 更新すべきか判定
        let should_insert = match existing_mpt_val {
            None => true, // MPTに存在しない（新規アカウント）なら絶対に挿入
            Some(old_rlp) => {
                // MPTに存在するなら、RLPの中身が変化している場合のみ挿入
                old_rlp != mpt_account_rlp_bytes
            }
        };

        // 更新
        if should_insert {
            tracing::debug!("更新: 0x{}", hex::encode(address));
            let _ = self.eth_trie.remove(address_hash.as_slice());
            self
                .eth_trie
                .insert(address_hash.as_slice(), mpt_account_rlp_bytes.as_slice())
                .unwrap();
        }
        //eth_trieのルートハッシュを取得
        let new_state_root = self.eth_trie.root_hash().unwrap();
        self.update_eth_trie(new_state_root);
    }



    pub fn add_cache(&mut self, address: &Address, mpt_account: &MptAccount) {
        let nonce = mpt_account.nonce;
        let balance = mpt_account.balance;
        let shash = mpt_account.storage_root;
        let code = self
            .code_storage
            .get(&mpt_account.code_hash)
            .cloned()
            .unwrap();
        //mpt_accoutのhashを取得
        let mut mpt_account_rlp_bytes = Vec::new();
        mpt_account.encode(&mut mpt_account_rlp_bytes);
        let mpt_account_hash = keccak256(mpt_account_rlp_bytes);
        let account = Account::make(nonce, balance, code, shash, mpt_account_hash);
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

    pub fn update_eth_trie(&mut self, state_root: B256) {
        let new_eth_trie = EthTrie::from(self.data.clone(), state_root).unwrap();
        self.eth_trie = new_eth_trie;
    }
}

#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct MptAccount {
    //MPT専用
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: B256,
    pub code_hash: B256,
}

impl MptAccount {
    pub fn new(nonce: u64, balance: U256, storage_root: B256, code_hash: B256) -> Self {
        Self {
            nonce,
            balance,
            storage_root,
            code_hash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub nonce: u64,
    pub balance: U256,
    pub storage: HashMap<U256, U256>,
    pub code: Vec<u8>,
    pub storage_hash: B256,
    pub account_hash: B256,
}

impl Default for Account {
    fn default() -> Self {
        Self::new()
    }
}

impl Account {
    pub fn new() -> Self {
        let empty_mpt = MptAccount::new(0, U256::ZERO, EMPTY_STORAGE_ROOT, EMPTY_CODE_HASH);

        // 2. RLPエンコードして正確なKeccak256ハッシュを動的に計算
        let mut rlp_bytes = Vec::new();
        empty_mpt.encode(&mut rlp_bytes);
        let correct_empty_hash = keccak256(&rlp_bytes);
        Self {
            nonce: 0u64,
            balance: U256::ZERO,
            storage: HashMap::new(),
            code: Vec::new(),
            storage_hash: EMPTY_STORAGE_ROOT,
            account_hash: correct_empty_hash,
        }
    }

    pub fn make(nonce: u64, balance: U256, code: Vec<u8>, shash: B256, account_hash: B256) -> Self {
        let storage = HashMap::<U256, U256>::new();
        Self {
            nonce,
            balance,
            storage,
            code,
            storage_hash: shash,
            account_hash,
        }
    }
}
