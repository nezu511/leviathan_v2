#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution, TransactionChecks, ContractCreation};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction};
use crate::evm::evm::EVM;


impl ContractCreation for LEVIATHAN {
    fn contract_creation(&mut self, state: &mut WorldState, substate: &mut SubState, sender: Address, origin: Address,
                         gas: U256, price: U256, eth: U256, init_code: Vec<u8>, depth: u32, solt: Option<U256>, sudo: bool
                         ) -> Result<(U256,Vec<u8>),(U256,Vec<u8>)> {

        todo!();

    }
}
