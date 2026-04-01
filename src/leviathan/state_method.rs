#![allow(dead_code)]

use crate::leviathan::structs::{ExecutionEnvironment, SubState};
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::leviathan_trait::State;
use alloy_primitives::{I256, U256};

impl State for WorldState {
    fn is_empty(&self, address: &Address) -> bool {
        //空だとtrue;
        if self.0.contains_key(address) {
            let account = self.0.get(address).unwrap();
            if account.nonce != 0
                || !account.balance.is_zero()
                || account.code.len() != 0
                || !account.storage.is_empty()
            {
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
        let storage = &account.unwrap().storage; //アカウントはevmを動かしてる時点で絶対にある！（addressがi_addressの場合)
        let value = storage.get(&key);
        return Some(value.cloned().unwrap_or(U256::from(0)));
    }

    fn get_nonce(&self, address: &Address) -> Option<u32> {
        if !self.0.contains_key(&address) {
            return None;
        }
        let account = self.0.get(&address);
        let nonce = account.unwrap().nonce;
        return Some(nonce);
    }

    //非推奨
    fn get_account(&self, address: &Address) -> Account {
        let account = self.0.get(&address);
        match account {
            Some(x) => return x.clone(),
            None => return Account::new(),
        }
    }

    fn set_balance(&mut self, address: &Address, value: U256) {
        let account = self
            .0
            .get_mut(&address)
            .expect("アカウントが存在しない.事前にadd_account");
        account.balance += value;
    }

    fn inc_nonce(&mut self, address: &Address) {
        //エラーが出るはずがない
        //事前にチェックして，&mut self系は呼ぶ
        let account = self
            .0
            .get_mut(&address)
            .expect("アカウントが存在しない.事前にadd_account");
        account.nonce += 1
    }

    fn dec_nonce(&mut self, address: &Address) {
        let account = self
            .0
            .get_mut(&address)
            .expect("[dec_nonce]: アカウントが存在しない");
        account.nonce += 1
    }

    fn set_storage(&mut self, address: &Address, key: U256, value: U256) {
        let account = self.0.get_mut(&address).unwrap();
        account.storage.insert(key, value);
    }

    fn remove_storage(&mut self, address: &Address, key: U256) {
        let account = self.0.get_mut(&address).unwrap();
        account.storage.remove(&key);
    }

    fn set_code(&mut self, address: &Address, code: Vec<u8>) {
        let account = self.0.get_mut(&address).unwrap();
        account.code = code;
    }

    fn send_eth(&mut self, from: &Address, to: &Address, eth: U256) -> Result<(), &'static str> {
        let from_account = self
            .0
            .get_mut(from)
            .expect("[send]: アカウントが存在しない");
        if from_account.balance >= eth {
            from_account.balance -= eth;
        } else {
            return Err("残高不足");
        }
        let to_account = self
            .0
            .get_mut(to)
            .expect("[send]: アカウントが存在しない");
        to_account.balance += eth;
        return Ok(());
    }

    fn buy_gas(
        &mut self,
        address: &Address,
        limit: U256,
        price: U256,
    ) -> Result<U256, &'static str> {
        let mut from_account = self
            .0
            .get_mut(address)
            .ok_or("送信元のアカウントが存在しない")?;
        let need_eth = limit.saturating_mul(price);
        if from_account.balance >= need_eth {
            from_account.balance -= need_eth;
        } else {
            return Err("残高不足");
        }
        return Ok(limit);
    }

    fn reset_storage(&mut self, address: &Address) {
        let account = self.0.get_mut(&address).unwrap();
        account.storage.clear();
    }

    fn delete_account(&mut self, address: &Address) {
        self.0.remove(&address);
    }

    fn add_account(&mut self, address: &Address, account: Account) {
        self.0.insert(address.clone(), account);
    }

    fn reset_balance(&mut self, address: &Address) {
        let account = self.0.get_mut(&address);
        match account {
            Some(x) => {
                x.balance = U256::ZERO;
            }
            None => (),
        }
    }
}
