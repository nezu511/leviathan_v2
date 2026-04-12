#![allow(dead_code)]

use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::evm_trait::Ofunction;
use crate::my_trait::leviathan_trait::{RoleBack, State};
use alloy_primitives::U256;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Action {
    Sstorage(Address, U256, U256),    //Address, Key, pre_value
    SendEth(Address, Address, U256), //from, to, eth
    AddNonce(Address),
    StoreCode(Address, Vec<u8>),
    AccountCreation(Address),
    DeleteAccount(Address, Account),
    ResetStorage(Address, HashMap<U256, U256>),
    SetBalance(Address, U256),
}

impl Action {
    pub fn push(self, leviathan: &mut LEVIATHAN, state: &WorldState) {
        let action = match self {
            Action::Sstorage(address, key, _) => {
                let pre_value = state
                    .get_storage_value(&address, &key)
                    .unwrap_or(U256::from(0));
                Action::Sstorage(address, key, pre_value)
            }

            Action::SendEth(_, _, _) => self,

            Action::AddNonce(_) => self,

            Action::StoreCode(address, _) => {
                let pre_code = state.get_code(&address).unwrap_or(Vec::<u8>::new());
                Action::StoreCode(address, pre_code)
            }

            Action::AccountCreation(_) => self,

            Action::DeleteAccount(address, _) => {
                let account = state.get_account(&address);
                Action::DeleteAccount(address, account)
            }

            Action::ResetStorage(address, _) => {
                let account = state.get_account(&address);
                let storage = account.storage.clone();
                Action::ResetStorage(address, storage)
            }

            Action::SetBalance(address, _) => {
                let pre_value = state.get_balance(&address).unwrap_or(U256::ZERO);
                Action::SetBalance(address, pre_value)
            }
        };
        leviathan.journal.push(action);
    }
}

impl RoleBack for LEVIATHAN {
    fn roleback(&mut self, state: &mut WorldState) -> Result<(), &'static str> {
        tracing::info!("ロールバック起動");
        //println!("{:?}", self.journal);
        while !self.journal.is_empty() {
            let action = self.journal.pop();
            match action.unwrap() {
                Action::Sstorage(address, key, pre_value) => {
                    if !pre_value.is_zero() {
                        state.set_storage(&address, key, pre_value);
                    } else {
                        state.remove_storage(&address, key);
                    }
                }

                Action::SendEth(from, to, eth) => {
                    state.send_eth(&to, &from, eth);
                }

                Action::AddNonce(address) => {
                    state.dec_nonce(&address);
                }

                Action::StoreCode(address, code) => {
                    state.set_code(&address, code);
                }

                Action::AccountCreation(address) => {
                    state.delete_account(&address);
                }

                Action::DeleteAccount(address, account) => {
                    state.add_account(&address, account);
                }

                Action::ResetStorage(address, storage) => {
                    for (key, value) in storage {
                        state.set_storage(&address, key, value);
                    }
                }

                Action::SetBalance(address, pre_value) => {
                    state.set_balance(&address, pre_value);
                }

                _ => return Err("不明なAction"),
            }
        }
        Ok(())
    }
}
