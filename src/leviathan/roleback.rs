#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, RoleBack};
use crate::my_trait::evm_trait::{Xi, Gfunction, Zfunction, Hfunction, Ofunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction, BlockHeader};
use crate::evm::evm::EVM;

#[derive(Debug,Clone)]
pub enum Action {
    Sstorage (Address,U256, U256),         //Address, pre_value, Key
    Send_eth (Address, Address, U256),       //from, to, eth
    Add_nonce (Address),
    Store_code (Address, Vec<u8>),
    Account_creation (Address),
    Delete_account (Address, Account),
    //SubStateのアクション
}


impl RoleBack for LEVIATHAN {
    fn roleback(&mut self, state: &mut WorldState)  -> Result<(), &'static str>{
        while self.0.len() > 0{
            let action = self.0.pop();
            match action.unwrap() {

                Action::Sstorage (address,pre_value, key) => {
                    if !pre_value.is_zero() {
                        state.set_storage(&address, key, pre_value);
                    }else{
                        state.remove_storage(&address, key);
                    }
                },

                Action::Send_eth (from, to, eth) => {
                    state.send_eth(&to, &from, eth);
                },

                Action::Add_nonce (address)    => {
                    state.dec_nonce(&address);
                },

                Action::Store_code (address, code) => {
                    state.set_code(&address, code);
                },

                Action::Account_creation (address) => {
                    state.delete_account(&address);
                },
                
                Action::Delete_account(address, account) => {
                    state.add_account(&address, account);
                },

                _ => return Err("不明なAction"),
            }
        }
        Ok(())
    }

}
