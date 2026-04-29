use tendermint_abci::{Application, ServerBuilder};
use tendermint_proto::abci::{
    ExecTxResult, RequestCheckTx, RequestFinalizeBlock, RequestInfo, ResponseCheckTx,
    ResponseCommit, ResponseFinalizeBlock, ResponseInfo,
};
use tracing::{info, Level};
use std::sync::Arc;
use std::sync::Mutex;

//自作構造体
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::VersionId;
use leviathan_v2::leviathan::world_state::{Account, WorldState};


#[derive(Clone)]
struct LeviathanApp {
    state: Arc<Mutex<WorldState>>,
    leviathan: Arc<Mutex<LEVIATHAN>>,
}

impl LeviathanApp {
    pub fn new(version: VersionId) -> Self {
        Self {
            state: Arc::new(Mutex::new(WorldState::new())),
            leviathan: Arc::new(Mutex::new(LEVIATHAN::new(version))),
        }
    }
}


impl Application for LeviathanApp {
    /// 1. Info: CometBFT起動時に呼ばれる状態同期
    fn info(&self, _req: RequestInfo) -> ResponseInfo {
        info!("[INFO] CometBFTからInfoリクエストを受信しました");
        ResponseInfo {
            data: "leviathan-mock".to_string(),
            version: "0.1.0".to_string(),
            app_version: 1,
            last_block_height: 0,
            last_block_app_hash: vec![].into(),
        }
    }

    /// 2. CheckTx: メモリープール投入前の単体検証
    fn check_tx(&self, req: RequestCheckTx) -> ResponseCheckTx {
        info!("[CHECK_TX] トランザクションを受信: {} bytes", req.tx.len());
        ResponseCheckTx {
            code: 0, // 0 = OK (受け入れ)
            ..Default::default()
        }
    }

    /// 3. FinalizeBlock: ブロックの実行とStateRootの計算 (旧 DeliverTx + Begin/EndBlock)
    fn finalize_block(&self, req: RequestFinalizeBlock) -> ResponseFinalizeBlock {
        info!("[FINALIZE_BLOCK] ブロック生成開始: {} 個のTXが含まれています", req.txs.len());
        
        // ブロック内の各トランザクションに対する実行結果（すべて成功として返す）
        let tx_results = req.txs.iter().map(|tx| {
            info!("   - TX実行: {} bytes", tx.len());
            ExecTxResult {
                code: 0, // 0 = 実行成功
                ..Default::default()
            }
        }).collect();

        // 実行完了後のStateRoot（AppHash）は、このメソッドで返す仕様に変更されました
        let dummy_app_hash = vec![0u8; 32];

        ResponseFinalizeBlock {
            tx_results,
            app_hash: dummy_app_hash.into(),
            ..Default::default()
        }
    }

    /// 4. Commit: 状態の永続化シグナル
    fn commit(&self) -> ResponseCommit {
        info!("[COMMIT] ブロックのステートを確定しました");
        ResponseCommit {
            retain_height: 0,
        }
    }
}

fn main() {
    // ログの初期化
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    info!("Leviathan ABCI Mock Serverを起動中...");

    let app = LeviathanApp::new(VersionId::Constantinople);
    
    let server = ServerBuilder::default()
        .bind("127.0.0.1:26658", app)
        .expect("サーバーのバインドに失敗しました");

    info!("ポート 26658 でCometBFTからの接続を待機しています...");
    server.listen().expect("サーバーの実行に失敗しました");
}
