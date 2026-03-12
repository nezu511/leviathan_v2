


pub trait Xi {
    puf fn evm_run(WorldState, SubState, ExecutionEnvironment) -> Result<(WorldState, SubState, ExecutionEnvironment, Vec<u8>), (WorldState, SubState, ExecutionEnvironment, Vec<u8>)> 
}
