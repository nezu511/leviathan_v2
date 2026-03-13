#![allow(dead_code)]

use crate::leviathan::world_state::{WorldState, Address, Account};
use primitive_types::U256;

pub struct SubState {
    a_des: Vec<Address>,
    a_log: Vec<Log>,
    a_touch: Vec<Address>, 
    a_reimburse: U256,
    a_access: Vec<Address>,
    a_access_storage: Vec<(Address,U256)>,
}

pub struct Log {
    address: Address,
    topic: Vec<U256>, //0~4個
    data: Vec<u8>,
}

