#![allow(dead_code)]
pub mod evm;
pub mod leviathan;
pub mod my_trait;
pub mod test;
use crate::evm::evm::EVM;
use crate::leviathan::world_state::{Account, Address, WorldState};

fn main() {
    println!("Hello, world!");
}
