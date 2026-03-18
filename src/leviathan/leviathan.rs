#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::evm_trait::{Xi, Gfunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};

#[derive(Debug,Clone)]
pub enum Action {
    Sstorage (Address,U256, U256),         //Address, pre_value, Key
    Send_eth (Address, Address, U256),       //from, to, eth
    Add_nonce (Address),
    Store_code (Address, Vec<u8>),
    Account_creation (Address),
    Child_evm (usize),
}


pub struct LEVIATHAN (Vec<Action>);


