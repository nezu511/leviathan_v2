use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment};


pub trait Xi {
    fn evm_run(&mut self, state: WorldState, substate: SubState, execution_environment: ExecutionEnvironment) -> Result<(WorldState, SubState, ExecutionEnvironment, Vec<u8>), (WorldState, SubState, ExecutionEnvironment, Vec<u8>)> ;
}
