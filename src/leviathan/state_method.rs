#![allow(dead_code)]

use crate::leviathan::structs::VersionId;
use crate::leviathan::world_state::{Account, WorldState};
use crate::my_trait::leviathan_trait::State;
use alloy_primitives::{U256, hex, Address};

impl State for WorldState {
    fn is_empty(&mut self, address: &Address) -> bool {
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
        true
    }

    fn is_dead(&mut self, version: VersionId, address: &Address) -> bool {
        //DEADだとtrue
        if version < VersionId::SpuriousDragon {
            !self.0.contains_key(address)
        } else {
            !self.0.contains_key(address) || self.is_empty(address)
        }
    }

    fn is_physically_exist(&mut self, address: &Address) -> bool {
        //存在してたらtrue
        self.0.contains_key(address)
    }

    fn is_storage_empty(&mut self, address: &Address) -> bool {
        //空だとtrue;
        let Some(account) = self.0.get(address) else {
            return true;
        };
        account.storage.is_empty()
    }

    fn get_balance(&mut self, address: &Address) -> Option<U256> {
        if !self.0.contains_key(address) {
            return None;
        }
        let account = self.0.get(address);
        let value = account.unwrap().balance;
        Some(value)
    }

    fn get_code(&mut self, address: &Address) -> Option<Vec<u8>> {
        if !self.0.contains_key(address) {
            return None;
        }
        let account = self.0.get(address);
        let code = account.unwrap().code.clone();
        Some(code)
    }

    fn get_storage_value(&mut self, address: &Address, key: &U256) -> Option<U256> {
        if !self.0.contains_key(address) {
            return None;
        }
        let account = self.0.get(address);
        let storage = &account.unwrap().storage; //アカウントはevmを動かしてる時点で絶対にある！（addressがi_addressの場合)
        let value = storage.get(key);
        Some(value.cloned().unwrap_or(U256::from(0)))
    }

    fn get_nonce(&mut self, address: &Address) -> Option<u64> {
        if !self.0.contains_key(address) {
            return None;
        }
        let account = self.0.get(address);
        let nonce = account.unwrap().nonce;
        Some(nonce)
    }

    //非推奨
    fn get_account(&mut self, address: &Address) -> Account {
        let account = self.0.get(address);
        match account {
            Some(x) => x.clone(),
            None => Account::new(),
        }
    }

    fn set_balance(&mut self, address: &Address, value: U256) {
        let account = self
            .0
            .get_mut(address)
            .expect("[set_balance]アカウントが存在しない.事前にadd_account");
        account.balance += value;
    }

    fn inc_nonce(&mut self, address: &Address) {
        //エラーが出るはずがない
        //事前にチェックして，&mut self系は呼ぶ
        let account = self
            .0
            .get_mut(address)
            .expect("[inc_nonce]アカウントが存在しない.事前にadd_account");
        tracing::info!("[inc_nonce]アドレス:0x{}", hex::encode(address.0)); //アドレス
        account.nonce += 1
    }

    fn dec_nonce(&mut self, address: &Address) {
        let account = self
            .0
            .get_mut(address)
            .expect("[dec_nonce]: アカウントが存在しない");
        account.nonce -= 1
    }

    fn set_storage(&mut self, address: &Address, key: U256, value: U256) {
        let account = self.0.get_mut(address).unwrap();
        account.storage.insert(key, value);
    }

    fn remove_storage(&mut self, address: &Address, key: U256) {
        let account = self.0.get_mut(address).unwrap();
        account.storage.remove(&key);
    }

    fn set_code(&mut self, address: &Address, code: Vec<u8>) {
        let account = self.0.get_mut(address).unwrap();
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
        let to_account = self.0.get_mut(to).expect("[send]: アカウントが存在しない");
        to_account.balance += eth;
        Ok(())
    }

    fn buy_gas(
        &mut self,
        address: &Address,
        limit: U256,
        price: U256,
    ) -> Result<U256, &'static str> {
        let from_account = self
            .0
            .get_mut(address)
            .ok_or("送信元のアカウントが存在しない")?;
        let need_eth = limit.saturating_mul(price);
        if from_account.balance >= need_eth {
            from_account.balance -= need_eth;
        } else {
            return Err("残高不足");
        }
        Ok(limit)
    }

    fn reset_storage(&mut self, address: &Address) {
        let account = self.0.get_mut(address).unwrap();
        account.storage.clear();
    }

    fn delete_account(&mut self, address: &Address) {
        self.0.remove(address);
    }

    fn add_account(&mut self, address: &Address, account: Account) {
        tracing::info!("add_accout: 0x{}", hex::encode(address.0));
        self.0.insert(address.clone(), account);
    }

    fn reset_balance(&mut self, address: &Address) {
        let account = self.0.get_mut(address);
        if let Some(x) = account {
            x.balance = U256::ZERO;
        }
    }
}
