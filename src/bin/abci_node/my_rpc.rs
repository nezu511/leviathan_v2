use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::ServerBuilder;
use std::sync::RwLock;
use std::sync::Arc;
use tendermint_rpc::{Client, HttpClient};
use alloy_primitives::hex;
use jsonrpsee::types::ErrorObjectOwned; 

use leviathan_v2::leviathan::world_state::WorldState;

#[rpc(server)]
pub trait EthApi {

    #[method(name = "eth_chainId")]
    async fn chain_id(&self) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_blockNumber")]
    async fn block_number(&self) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_sendRawTransaction")]
    async fn send_raw_transaction(&self, tx_bytes: String) -> jsonrpsee::core::RpcResult<String>;
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
    async fn chain_id(&self) -> jsonrpsee::core::RpcResult<String> {
        Ok("0xC5691481E".to_string())
    }

    async fn block_number(&self) -> jsonrpsee::core::RpcResult<String> {
        let state = self.state.read().unwrap();
        let block_number = state.current_block_number();

        Ok(format!("0x{}",block_number).to_string())
    }

    async fn send_raw_transaction(&self, tx_bytes: String) -> jsonrpsee::core::RpcResult<String> {
        let tx_hex = tx_bytes.trim_start_matches("0x");

        // 1. デコードエラー（-32602: Invalid params）
        let tx_data = hex::decode(tx_hex).map_err(|e| {
            ErrorObjectOwned::owned(-32602, format!("Hex decode error: {}", e), None::<()>)
        })?;

        // 2. CometBFTクライアント作成エラー（-32603: Internal error）
        let client = HttpClient::new("http://127.0.0.1:26657").map_err(|e| {
            ErrorObjectOwned::owned(-32603, format!("CometBFT Client error: {}", e), None::<()>)
        })?;

        // 3. CometBFTへの送信エラー（-32603: Internal error）
        let response = client.broadcast_tx_sync(tx_data).await.map_err(|e| {
            ErrorObjectOwned::owned(-32603, format!("Broadcast error: {}", e), None::<()>)
        })?;

        Ok(format!("0x{}", hex::encode(response.hash)))
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
