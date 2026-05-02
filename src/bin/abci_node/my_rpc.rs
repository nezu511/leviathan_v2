use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::ServerBuilder;
use std::sync::RwLock;
use std::sync::Arc;

use leviathan_v2::leviathan::world_state::WorldState;

#[rpc(server)]
pub trait EthApi {

    #[method(name = "eth_chainId")]
    async fn chain_id(&self) -> Result<String, jsonrpsee::core::Error>;

    #[method(name = "eth_blockNumber")]
    async fn block_number(&self) -> Result<String, jsonrpsee::core::Error>;
}

pub struct LeviathanRPC {
    state: Arc<RwLock<WorldState>>,
}

impl LeviathanRPC {
    pub fn new(state: Arc<RwLock<WorldState>>) -> Self {
        Self {state}
    }
}

#[async_trait::async_trait]
impl EthApiServer for LeviathanRPC {
    async fn chain_id(&self) -> Result<String, jsonrpsee::core::Error> {
        Ok("0xC5691481E".to_string())
    }

    async fn block_number(&self) -> Result<String, jsonrpsee::core::Error> {
        Ok("0x0".to_string())
    }
}



pub async fn run_rpc_server(state: Arc<RwLock<WorldState>>) {
    // サーバーのビルド (ポート8545)
    let server = ServerBuilder::default()
        .build("127.0.0.1:8545")
        .await
        .expect("RPCサーバーの起動に失敗しました");

    // 実装インスタンスの作成とRPCモジュール化
    let rpc_impl = LeviathanRPC::new(state);
    let handle = server.start(rpc_impl.into_rpc());

    tracing::info!("JSON-RPCサーバーを 127.0.0.1:8545 で起動しました");

    // サーバーが終了しないように待機
    handle.stopped().await;
}
