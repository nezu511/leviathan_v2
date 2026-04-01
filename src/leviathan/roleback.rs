#![allow(dead_code)]

use crate::evm::evm::EVM;
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::{BlockHeader, ExecutionEnvironment, Log, SubState, Transaction};
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::evm_trait::{Gfunction, Hfunction, Ofunction, Xi, Zfunction};
use crate::my_trait::leviathan_trait::{RoleBack, State};
use alloy_primitives::{I256, U256};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Action {
    Sstorage(Address, U256, U256),    //Address, Key, pre_value
    Send_eth(Address, Address, U256), //from, to, eth
    Add_nonce(Address),
    Store_code(Address, Vec<u8>),
    Account_creation(Address),
    Delete_account(Address, Account),
    Reset_storage(Address, HashMap<U256, U256>),
    Set_balance(Address, U256),
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

            Action::Send_eth(_, _, _) => self,

            Action::Add_nonce(_) => self,

            Action::Store_code(address, _) => {
                let pre_code = state.get_code(&address).unwrap_or(Vec::<u8>::new());
                Action::Store_code(address, pre_code)
            }

            Action::Account_creation(_) => self,

            Action::Delete_account(address, _) => {
                let account = state.get_account(&address);
                Action::Delete_account(address, account)
            }

            Action::Reset_storage(address, _) => {
                let account = state.get_account(&address);
                let storage = account.storage.clone();
                Action::Reset_storage(address, storage)
            }

            Action::Set_balance(address, _) => {
                let pre_value = state.get_balance(&address).unwrap_or(U256::ZERO);
                Action::Set_balance(address, pre_value)
            }
        };
        leviathan.journal.push(action);
    }
}

impl RoleBack for LEVIATHAN {
    fn roleback(&mut self, state: &mut WorldState) -> Result<(), &'static str> {
        println!("ロールバック起動");
        println!("{:?}", self.journal);
        while self.journal.len() > 0 {
            let action = self.journal.pop();
            match action.unwrap() {
                Action::Sstorage(address, key, pre_value) => {
                    if !pre_value.is_zero() {
                        state.set_storage(&address, key, pre_value);
                    } else {
                        state.remove_storage(&address, key);
                    }
                }

                Action::Send_eth(from, to, eth) => {
                    state.send_eth(&to, &from, eth);
                }

                Action::Add_nonce(address) => {
                    state.dec_nonce(&address);
                }

                Action::Store_code(address, code) => {
                    state.set_code(&address, code);
                }

                Action::Account_creation(address) => {
                    state.delete_account(&address);
                }

                Action::Delete_account(address, account) => {
                    state.add_account(&address, account);
                }

                Action::Reset_storage(address, storage) => {
                    for (key, value) in storage {
                        state.set_storage(&address, key, value);
                    }
                }

                Action::Set_balance(address, pre_value) => {
                    state.set_balance(&address, pre_value);
                }

                _ => return Err("不明なAction"),
            }
        }
        Ok(())
    }
}
