#![allow(dead_code)]

use crate::leviathan::structs::VersionId;
use crate::leviathan::world_state::{Account, WorldState2, MptAccount, EMPTY_STORAGE_ROOT, EMPTY_CODE_HASH};
use crate::my_trait::leviathan_trait::State;
use alloy_primitives::{U256, hex, Address, b256, B256, keccak256};
use eth_trie::Trie;
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

}

