use alloy_primitives::{Address, U256, hex};
use alloy_rlp::Decodable;
use eth_trie::{DB, Trie};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};
use tendermint_abci::Application;
use tendermint_proto::abci::{
    RequestCheckTx, RequestFinalizeBlock, RequestInfo, RequestInitChain, ResponseCheckTx,
    ResponseCommit, ResponseFinalizeBlock, ResponseInfo, ResponseInitChain,
    RequestPrepareProposal, ResponsePrepareProposal
};
use tracing::info;

//自作構造体
use crate::req_execution::PI;
use crate::tx_check::Tx_Checker;
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::{Transaction, VersionId};
use leviathan_v2::leviathan::world_state::{Account, MptAccount, WorldState};

#[derive(Clone)]
pub struct LeviathanApp {
    pub state: Arc<RwLock<WorldState>>,
    pub leviathan: Arc<Mutex<LEVIATHAN>>,
    pub version: VersionId,
    pub cache: Arc<RwLock<LruCache<Address, MptAccount>>>,
}

impl LeviathanApp {
    pub fn new(version: VersionId, db_path: &str) -> Self {
        Self {
            state: Arc::new(RwLock::new(WorldState::new(db_path))),
            leviathan: Arc::new(Mutex::new(LEVIATHAN::new(version))),
            version,
            cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()))),
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

            //LeviathanApp.cacheをクリアー
            let mut cache = self.cache.write().unwrap();
            cache.clear();
            return ResponseCommit { retain_height: 0 };
        };

        tracing::error!("RocksDBへのFlushに失敗: {:?}", e);
        panic!("Critical Database Error: {}", e);
    }

    fn init_chain(&self, _req: RequestInitChain) -> ResponseInitChain {
        tracing::info!("[INIT_CHAIN] ブロックチェーンの創世を開始します...");

        // 1. ジェネシスアドレスを決める
        let genesis_address_bytes =
            hex::decode("c755095A6D433b4E744f706881D5d7E0D84237B5").unwrap();
        let genesis_address = Address::from_slice(&genesis_address_bytes);

        // 2. 1万ETH（10000 * 10^18 wei）を付与
        let one_eth = U256::from(10_u64).pow(U256::from(18));
        let genesis_balance = one_eth * U256::from(10000);

        // 3. アカウントを作成してWorldStateに書き込む
        let mut genesis_account = Account::new();
        genesis_account.balance = genesis_balance;

        let mut state = self.state.write().unwrap();
        // ★ WorldState側に init_mpt_account 等を使ってアカウントを登録する処理を呼び出す
        state.init_mpt_account(&genesis_address, &genesis_account);

        // 4. 初期のState Root（AppHash）を取得してCometBFTに教える
        let app_hash = state.eth_trie.root_hash().unwrap().0.to_vec();

        tracing::info!(
            "[INIT_CHAIN] ジェネシス完了。AppHash: 0x{}",
            hex::encode(&app_hash)
        );

        ResponseInitChain {
            app_hash: app_hash.into(),
            ..Default::default()
        }
    }

    fn prepare_proposal(&self, req: RequestPrepareProposal) -> ResponsePrepareProposal {
        tracing::info!(
            "[PREPARE_PROPOSAL] ブロック提案の準備を開始します。候補TX数: {}, 最大許容サイズ: {} bytes",
            req.txs.len(),
            req.max_tx_bytes
            );

        // ※トランザクションの選別や並び替えのロジックを組み込む
        ResponsePrepareProposal {
            txs: req.txs,
        }
}

}
