mod my_abci;
mod my_rpc;
mod req_execution;
mod tx_check;
mod utils;

use http::{Method, header};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};
use tendermint_abci::ServerBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{Level, info};
use tracing_subscriber::EnvFilter;

//自作構造体
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::VersionId;
use leviathan_v2::leviathan::world_state::WorldState;
use my_abci::LeviathanApp;
use my_rpc::run_rpc_server;

#[tokio::main]
async fn main() {
    /*/ ログの初期化
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()) // 環境変数を読み込む
        .init();
    */
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let db_path = "data/leviathan_db";
    let state = Arc::new(RwLock::new(WorldState::new(db_path)));
    let leviathan = Arc::new(Mutex::new(LEVIATHAN::new(VersionId::Constantinople)));
    std::fs::create_dir_all(db_path).expect("DBディレクトリの作成に失敗しました");

    //LeviathanRPCを作成
    let state_rpc = state.clone();
    info!("Leviathan RPC Serverを起動中...");
    tokio::spawn(async move {
        run_rpc_server(Arc::clone(&state_rpc), VersionId::Constantinople).await;
    });
    info!("Leviathan RPC Serverを起動");

    let state_abci = state.clone();
    info!("Leviathan ABCI Serverを起動中...");
    let app = LeviathanApp {
        state: Arc::clone(&state_abci),
        leviathan: Arc::clone(&leviathan),
        version: VersionId::Constantinople,
        cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()))),
    };

    let server = ServerBuilder::default()
        .bind("127.0.0.1:26658", app)
        .expect("サーバーのバインドに失敗しました");

    info!("ポート 26658 でCometBFTからの接続を待機しています...");
    server.listen().expect("サーバーの実行に失敗しました");
}
