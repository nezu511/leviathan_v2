#![allow(dead_code)]

use crate::leviathan::structs::VersionId;
use crate::leviathan::world_state::{Account, WorldState2, MptAccount, EMPTY_STORAGE_ROOT, EMPTY_CODE_HASH};
use crate::my_trait::leviathan_trait::State;
use alloy_primitives::{U256, hex, Address, b256, B256, keccak256};
use eth_trie::{Trie, EthTrie};
use alloy_rlp::{Decodable};


impl State for WorldState2 {
    fn add_account(&mut self, address: &Address, account: Account) {
        self.cache.insert(address.clone(), account);
    }

    fn is_empty(&mut self, address: &Address) -> bool {
        //まずcacheに存在するか確認
        if self.cache.contains_key(address) {
            let account = self.cache.get(address).unwrap();
            if account.nonce != 0
                || !account.balance.is_zero()
                || account.code.len() != 0
                || account.storage_hash != EMPTY_STORAGE_ROOT
            {
                return false;
            }
            return true;
        }else{
            //次にMPTに存在するか確認
            let address_hash = keccak256(address);
            let result = self.eth_trie.get(address_hash.as_slice()).unwrap();

            match result {
                Some(rlp_bytes) =>{
                    let mut slice = rlp_bytes.as_slice();
                    let Ok(account) = MptAccount::decode(&mut slice) else {
                        tracing::warn!("[is_empty] MptAccount::decodeでエラー");
                        return false;
                    };
                    self.add_cache(address, &account);

                    if account.nonce != 0
                        || !account.balance.is_zero()
                            || account.code_hash != EMPTY_CODE_HASH
                            || account.storage_root != EMPTY_STORAGE_ROOT
                            {
                                return false;
                            }
                    return true;
                }

                None => return true,
            }
        }
    }

    fn is_dead(&mut self, version: VersionId, address: &Address) -> bool {
        //DEADだとtrue
        if version < VersionId::SpuriousDragon {
            if self.cache.contains_key(address) {   //chaceを調査
                return false
            }else{
                //MPTを調査
                let Some(account) = self.contain_mpt(address) else{
                    return true;
                };
                self.add_cache(address, &account);
                return false;
            }
        } else {
            return self.is_empty(address);
        }
    }

    fn is_physically_exist(&mut self, address: &Address) -> bool {
        //存在してたらtrue
        if self.cache.contains_key(address) {   //chaceを調査
            return true
        }else{
            //MPTを調査
            let Some(account) = self.contain_mpt(address) else{
                return false;
            };
            self.add_cache(address, &account);
            return true;
        }
    }


    fn is_storage_empty(&mut self, address: &Address) -> bool {
        //空だとtrue;
        //cache調査
        if let Some(cache_account) = self.cache.get(address) {
            return cache_account.storage_hash == EMPTY_STORAGE_ROOT 
        }
        //MPTを調査
        let Some(mpt_account) = self.contain_mpt(address) else{
            return true;
        };
        self.add_cache(address, &mpt_account);
        mpt_account.storage_root == EMPTY_STORAGE_ROOT 
    }

    fn get_balance(&mut self, address: &Address) -> Option<U256> {
        //cache調査
        if let Some(cache_account) = self.cache.get(address) {
            return Some(cache_account.balance);
        }
        //MPTを調査
        let Some(mpt_account) = self.contain_mpt(address) else{
            return None;
        };
        self.add_cache(address, &mpt_account);
        Some(mpt_account.balance)
    }

    fn get_code(&mut self, address: &Address) -> Option<Vec<u8>> {
        //cache調査
        if let Some(cache_account) = self.cache.get(address) {
            return Some(cache_account.code.clone());
        }
        //MPTを調査
        let Some(mpt_account) = self.contain_mpt(address) else{
            return None;
        };
        self.add_cache(address, &mpt_account);
        let code = self.code_storage.get(&mpt_account.code_hash).cloned().unwrap();
        Some(code)
    }

    fn get_storage_value(&mut self, address: &Address, key: &U256) -> Option<U256> {
        if !self.cache.contains_key(address) {
            let Some(mpt_account) = self.contain_mpt(address) else {
                return None; // アカウント自体が存在しない場合は None
            };
            self.add_cache(address, &mpt_account);
        }

        let cache_account = self.cache.get_mut(address).unwrap();

        if let Some(value) = cache_account.storage.get(key).cloned() {
            return Some(value);
        }

        if cache_account.storage_hash != EMPTY_STORAGE_ROOT {  //ストレージが空か確認
            let Ok(storage_trie) = EthTrie::from(self.data.clone(), cache_account.storage_hash) else{
                tracing::warn!("[get_storage_value] EthTrie::fromでエラー");
                return None;
            };
            let key_byte: [u8;32] = key.to_be_bytes();
            let key_hash = keccak256(key_byte);
            let Some(val) = storage_trie.get(key_hash.as_slice()).unwrap() else {
                return None;
            };
            //値をcacheに保存
            let mut slice = val.as_slice();
            let Ok(val) = U256::decode(&mut slice) else {
                tracing::warn!("[get_storage_value] U256::decodeでエラー");
                return None;
            };
            cache_account.storage.insert(*key, val);
            return Some(val);
        }
        return None;
    }

    
    fn get_nonce(&mut self, address: &Address) -> Option<u64> {
        //cache調査
        if let Some(cache_account) = self.cache.get(address) {
            return Some(cache_account.nonce);
        }
        //MPTを調査
        let Some(mpt_account) = self.contain_mpt(address) else{
            return None;
        };
        self.add_cache(address, &mpt_account);
        Some(mpt_account.nonce)
    }


    //非推奨
    fn get_account(&mut self, address: &Address) -> Account {
        //cache調査
        if let Some(cache_account) = self.cache.get(address).cloned() {
            return cache_account;
        }
        //MPTを調査
        let Some(mpt_account) = self.contain_mpt(address) else{
            return Account::new();
        };
        self.add_cache(address, &mpt_account);
        if let Some(cache_account) = self.cache.get(address).cloned() {
            return cache_account;
        };
        return Account::new();
    }


}

