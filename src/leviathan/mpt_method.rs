#![allow(dead_code)]

use crate::leviathan::structs::VersionId;
use crate::leviathan::world_state::{Account, WorldState, MptAccount, EMPTY_STORAGE_ROOT, EMPTY_CODE_HASH};
use crate::my_trait::leviathan_trait::State;
use alloy_primitives::{U256, hex, Address, b256, B256, keccak256};
use eth_trie::{Trie, EthTrie};
use alloy_rlp::{Decodable};


impl State for WorldState {
    fn add_account(&mut self, address: &Address, account: Account) {
        tracing::info!(
            address =  format_args!("0x{}", hex::encode(address.0)),
            "[add_acout]"
            );
        self.cache.insert(address.clone(), account);
    }

    fn is_empty(&mut self, address: &Address) -> bool {
        //まずcacheに存在するか確認
        if self.cache.contains_key(address) {
            let account = self.cache.get(address).unwrap();
            if account.nonce != 0
                || !account.balance.is_zero()
                || account.code.len() != 0
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

    //=============Setterメソッド===============
    //事前にチェックして，&mut self系は呼ぶ
    //パニックは絶対に生じない
    fn add_balance(&mut self, address: &Address, value: U256) {
        let cache_account = self.cache.get_mut(address)
            .expect("[set_balance]アカウントが存在しない.事前にadd_account");
        cache_account.balance += value;
    }

    fn inc_nonce(&mut self, address: &Address) {
        let cache_account = self.cache.get_mut(address)
            .expect("[inc_nonce]アカウントが存在しない.事前にadd_account");
        tracing::info!("[inc_nonce]アドレス:0x{}", hex::encode(address.0)); //アドレス
        cache_account.nonce += 1
    }

    fn dec_nonce(&mut self, address: &Address) {
        let cache_account = self.cache.get_mut(address)
            .expect("[dec_nonce]: アカウントが存在しない");
        cache_account.nonce -= 1
    }

    fn set_storage(&mut self, address: &Address, key: U256, value: U256) {
        let cache_account = self.cache.get_mut(address)
            .expect("[set_storage] アカウントが存在しない");
        cache_account.storage.insert(key, value);
    }

    fn remove_storage(&mut self, address: &Address, key: U256) {
        let cache_account = self.cache.get_mut(address)
            .expect("[remove_storage] アカウントが存在しない");
        cache_account.storage.insert(key, U256::ZERO);
    }

    fn set_code(&mut self, address: &Address, code: Vec<u8>) {
        let cache_account = self.cache.get_mut(address)
            .expect("[set_code] アカウントが存在しない");
        cache_account.code = code;
    }

    fn send_eth(&mut self, from: &Address, to: &Address, eth: U256) -> Result<(), &'static str> {
        let cache_from_account = self.cache.get_mut(from)
            .expect("[send_eth]: fromアカウントが存在しない");
        if cache_from_account.balance >= eth {
            cache_from_account.balance -= eth;
        } else {
            return Err("残高不足"); //事前チェックを済ませているため発生しない
        }
        let cache_to_account = self.cache.get_mut(to).expect("[send_eth]: toアカウントが存在しない");
        cache_to_account.balance += eth;
        Ok(())
    }

    fn buy_gas(
        &mut self,
        address: &Address,
        limit: U256,
        price: U256,
    ) -> Result<U256, &'static str> {
        let cache_from_account = self.cache.get_mut(address)
            .expect("[buy_gas]送信元のアカウントが存在しない");
        let need_eth = limit.saturating_mul(price);
        if cache_from_account.balance >= need_eth {
            cache_from_account.balance -= need_eth;
        } else {
            return Err("残高不足");
        }
        Ok(limit)
    }

    fn reset_storage(&mut self, address: &Address) {
        //アカウントがcacheにある前提
        let cache_account = self.cache.get_mut(address)
            .expect("[reset_storage] アカウントが存在しない");
        cache_account.storage_hash = EMPTY_STORAGE_ROOT;
    }

    fn delete_account(&mut self, address: &Address) {
        self.cache.remove(address);
    }


    fn reset_balance(&mut self, address: &Address) {
        let cache_account = self.cache.get_mut(address)
            .expect("[reset_balance]: アカウントが存在しない");
        cache_account.balance = U256::ZERO;
    }
}

