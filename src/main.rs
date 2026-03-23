#![allow(dead_code)]
pub mod test { pub mod test_parser; }
pub mod leviathan;
pub mod my_trait;
pub mod evm;
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::evm::evm::EVM;

fn main() {
    println!("Hello, world!");
}
