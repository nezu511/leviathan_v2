use alloy_rlp::Decodable;
use eth_trie::{DB, Trie};
use std::sync::Arc;
use std::sync::{RwLock, Mutex};
use tendermint_abci::{Application, ServerBuilder};
use tendermint_proto::abci::{
    RequestCheckTx, RequestFinalizeBlock, RequestInfo, ResponseCheckTx,
    ResponseCommit, ResponseFinalizeBlock, ResponseInfo,
};
use tracing::{Level, info};

//自作構造体
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::{Transaction, VersionId};
use leviathan_v2::leviathan::world_state::WorldState;
use crate::my_rpc::run_rpc_server;
use crate::req_execution::PI;
use crate::tx_check::Tx_Checker;


#[derive(Clone)]
pub struct LeviathanApp {
    pub state: Arc<RwLock<WorldState>>,
    pub leviathan: Arc<Mutex<LEVIATHAN>>,
    pub version: VersionId,
}

impl LeviathanApp {
    pub fn new(version: VersionId, db_path: &str) -> Self {
        Self {
            state: Arc::new(RwLock::new(WorldState::new(db_path))),
            leviathan: Arc::new(Mutex::new(LEVIATHAN::new(version))),
            version,
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

        let mut raw_tx_slice = req.tx.as_ref();

        match Transaction::decode(&mut raw_tx_slice) {
            Ok(transaction) => {
                tracing::info!(
                    "[CHECK_TX] デコード成功: Nonce={}, GasLimit={}",
                    transaction.t_nonce,
                    transaction.t_gas_limit
                );

                let is_valid = self.validate_transaction(&transaction);
                if !is_valid {
                    return ResponseCheckTx {
                        code: 1, // 不正なトランザクションは弾く
                        log: "Validation Failed".to_string(),
                        ..Default::default()
                    };
                }

                ResponseCheckTx {
                    code: 0,
                    ..Default::default()
                }
            }
            Err(err) => {
                // デコード失敗（スパムやEthereum互換ではないフォーマット）
                tracing::warn!("[CHECK_TX] RLPデコード失敗: {:?}", err);

                // codeを非ゼロ（例: 1）にしてCometBFTに弾かせる
                ResponseCheckTx {
                    code: 1,
                    log: format!("RLP Decode Error: {}", err),
                    ..Default::default()
                }
            }
        }
    }

    /// 3. FinalizeBlock: ブロックの実行とStateRootの計算 (旧 DeliverTx + Begin/EndBlock)
    fn finalize_block(&self, req: RequestFinalizeBlock) -> ResponseFinalizeBlock {
        info!(
            "[FINALIZE_BLOCK] ブロック生成開始: {} 個のTXが含まれています",
            req.txs.len()
        );

        // ブロック内の各トランザクションに対する実行結果（すべて成功として返す）
        let tx_results = self.tx_execution(&req);

        let new_state_root = {
            let mut state = self.state.write().unwrap();
            state.eth_trie.root_hash().unwrap()
        };

        // 実行完了後のStateRoot（AppHash）は、このメソッドで返す仕様に変更されました
        let dummy_app_hash = new_state_root.0.to_vec();

        ResponseFinalizeBlock {
            tx_results,
            app_hash: dummy_app_hash.into(),
            ..Default::default()
        }
    }

    /// 4. Commit: 状態の永続化シグナル
    fn commit(&self) -> ResponseCommit {
        tracing::info!("[COMMIT] ステートを確定します．");
        let state = self.state.read().unwrap();
        let Err(e) = state.data.flush() else {
            tracing::info!("[COMMIT] 無事書き込み成功");
            return ResponseCommit { retain_height: 0 };
        };

        tracing::error!("RocksDBへのFlushに失敗: {:?}", e);
        panic!("Critical Database Error: {}", e);
        ResponseCommit { retain_height: 0 }
    }
}
