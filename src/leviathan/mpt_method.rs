#![allow(dead_code)]

use crate::leviathan::structs::VersionId;
use crate::leviathan::world_state::{Account, WorldState};
use crate::my_trait::leviathan_trait::State;
use alloy_primitives::{U256, hex, Address};

impl State for WorldState2 {
    fn add_account(&mut self, address: &Address, account: Account) {
        self.cache.insert(address.clone(), account);
    }

    fn is_empty(&self, address: &Address) -> bool {
        //まずcacheに存在するか確認
        if self.cache.contains_key(address) {
            let account = self.0.get(address).unwrap();
            if account.nonce != 0
                || !account.balance.is_zero()
                || account.code.len() != 0
                || !account.storage.is_empty()
            {
                return false;
            }
        }else{
            //次にMPTに存在するか確認



