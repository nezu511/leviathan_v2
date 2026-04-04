use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::{ExecutionEnvironment, SubState};
use crate::leviathan::world_state::{Account, Address, WorldState};
use alloy_primitives::{I256, U256};

pub trait Xi {
    fn evm_run(
        &mut self,
        leviathan: &mut LEVIATHAN,
        state: &mut WorldState,
        substate: &mut SubState,
        execution_environment: &mut ExecutionEnvironment,
    ) -> Result<Vec<u8>, Option<Vec<u8>>>;

    //Ok()：正常停止
    //Err(None) => Z関数による停止
    //Err(Some(Vec<u8>)) => REVERTによる停止
}

pub trait Gfunction {
    //返り値は消費ガス量
    fn gas(
        &mut self,
        opcode: u8,
        substate: &SubState,
        state: &WorldState,
        execution_environment: &ExecutionEnvironment,
    ) -> U256;

    fn extension_cost(&mut self, offset: U256, size: U256) -> U256;

    fn is_account_access(&mut self, data: U256, substate: &SubState) -> U256;
}

pub trait Zfunction {
    //Z関数による安全性を確認
    fn is_safe(
        &mut self,
        opcode: u8,
        substate: &SubState,
        state: &WorldState,
        execution_environment: &ExecutionEnvironment,
    ) -> bool;
}

pub trait Ofunction {
    //状態遷移
    fn execution(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    ) -> Option<bool>;
    //Noneのときは継続
    //Some(true)：Revert
    //Some(false):STOP, RETURN, SELFDESTRUCT

    fn pop(&mut self) -> U256;
    fn push(&mut self, val: U256);




    fn arithmetic_opcodes(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn comparison_bitwise_opcodes(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn keccak256_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn environmental_info_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn block_info_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn mload_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn mstore_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn mstore8_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );
    fn create_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn call_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn callcode_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn delegatecall_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn create2_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn staticcall_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );

    fn log_opcode(
        &mut self,
        opcode: u8,
        leviathan: &mut LEVIATHAN,
        substate: &mut SubState,
        state: &mut WorldState,
        execution_environment: &ExecutionEnvironment,
    );
  
}

pub trait Hfunction {
    fn evm_stop(&mut self, opcode: u8) -> Result<(), Option<Vec<u8>>>;
}
