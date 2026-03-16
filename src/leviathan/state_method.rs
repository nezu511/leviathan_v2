#![allow(dead_code)]

use crate::my_trait::leviathan_trait::{State};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};
use primitive_types::U256;


impl State for WorldState {
    fn is_empty(&self,address: &Address) -> bool {   //空だとtrue;
        if self.0.contains_key(address) {
            let account = self.0.get(address).unwrap();
            if account.nonce != 0 ||  !account.balance.is_zero() || account.code.len() != 0 {
                return false;
            }
        }
        return true;
    }
                                            
    fn get_balance(&self, address: &Address) -> Option<U256> {
        if !self.0.contains_key(&address) {
            return None;
        }
        let account = self.0.get(&address);
        let value = account.unwrap().balance.clone();
        return Some(value);
    }

    fn get_code(&self, address: &Address) -> Option<Vec<u8>> {
        if !self.0.contains_key(&address) {
            return None;
        }
        let account = self.0.get(&address);
        let code = account.unwrap().code.clone();
        return Some(code);
    }


    fn get_storage_value(&self, address: &Address, key: &U256) -> Option<U256> {
        if !self.0.contains_key(&address) {
            return None;
        }
        let account = self.0.get(&address);
        let storage = &account.unwrap().storage;      //アカウントはevmを動かしてる時点で絶対にある！（addressがi_addressの場合)
        let value = storage.get(&key);
        return Some(value.cloned().unwrap_or(U256::from(0)));
    }
}






