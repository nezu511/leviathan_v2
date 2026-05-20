use alloy_consensus::transaction::Recovered;
use alloy_consensus::{
    Block, BlockBody, Header as BlockHeader, Receipt, ReceiptWithBloom, Signed, TxEnvelope,
};
use alloy_primitives::{Address, B256, Signature, TxKind, U256, hex, keccak256};
use alloy_rlp::{Decodable, Encodable, Header};
use alloy_rpc_types::{
    Block as RpcBlock, BlockNumberOrTag, BlockTransactions, Filter, Header as RpcHeader,
    Transaction as RPCTransaction, TransactionReceipt, TransactionRequest,
};
use bytes::BytesMut;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::ServerBuilder;
use jsonrpsee::types::ErrorObjectOwned;
use secp256k1::{
    Message, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use sha3::{Digest, Keccak256};
use std::sync::Arc;
use std::sync::RwLock;
use tendermint_rpc::{Client, HttpClient};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::utils::{format_to_rpc_log, get_sender, is_bloom_match, is_exact_match};
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::{Transaction, VersionId};
use leviathan_v2::leviathan::world_state::WorldState;
use leviathan_v2::my_trait::leviathan_trait::TransactionExecution;

#[rpc(server)]
pub trait EthApi {
    #[method(name = "eth_chainId")]
    async fn chain_id(&self) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_blockNumber")]
    async fn block_number(&self) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_sendRawTransaction")]
    async fn send_raw_transaction(&self, tx_bytes: String) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_getTransactionReceipt")]
    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> jsonrpsee::core::RpcResult<Option<TransactionReceipt>>;

    #[method(name = "eth_getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<String>,
    ) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_getTransactionByHash")]
    async fn get_transaction_by_hash(
        &self,
        tx_hash: B256,
    ) -> jsonrpsee::core::RpcResult<Option<RPCTransaction>>;

    #[method(name = "eth_getBalance")]
    async fn get_balance(
        &self,
        address: Address,
        block: Option<String>,
    ) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_getBlockByNumber")]
    async fn get_block_by_number(
        &self,
        number: BlockNumberOrTag,
        full_transactions: bool,
    ) -> jsonrpsee::core::RpcResult<Option<RpcBlock>>;

    #[method(name = "eth_call")]
    async fn eth_call(
        &self,
        request: alloy_rpc_types::TransactionRequest,
        block_number: Option<alloy_rpc_types::BlockNumberOrTag>,
    ) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_getLogs")]
    async fn get_logs(&self, filter: Filter)
    -> Result<Vec<alloy_rpc_types::Log>, ErrorObjectOwned>;
}

pub struct LeviathanRPC {
    state: Arc<RwLock<WorldState>>,
    pub version: VersionId,
}

impl LeviathanRPC {
    pub fn new(state: Arc<RwLock<WorldState>>, version: VersionId) -> Self {
        Self { state, version }
    }
}

#[async_trait::async_trait]
impl EthApiServer for LeviathanRPC {
    async fn chain_id(&self) -> jsonrpsee::core::RpcResult<String> {
        Ok("0x539".to_string()) //1337
    }

    async fn block_number(&self) -> jsonrpsee::core::RpcResult<String> {
        let state = self.state.read().unwrap();
        let block_number = state.current_block_number();

        Ok(format!("0x{:x}", block_number).to_string())
    }

    async fn send_raw_transaction(&self, tx_bytes: String) -> jsonrpsee::core::RpcResult<String> {
        let tx_hex = tx_bytes.trim_start_matches("0x");

        // 1. デコードエラー（-32602: Invalid params）
        let tx_data = hex::decode(tx_hex).map_err(|e| {
            ErrorObjectOwned::owned(-32602, format!("Hex decode error: {}", e), None::<()>)
        })?;

        let eth_tx_hash = alloy_primitives::keccak256(&tx_data);

        // 2. CometBFTクライアント作成エラー（-32603: Internal error）
        let client = HttpClient::new("http://127.0.0.1:26657").map_err(|e| {
            ErrorObjectOwned::owned(-32603, format!("CometBFT Client error: {}", e), None::<()>)
        })?;

        // 3. CometBFTへの送信エラー（-32603: Internal error）
        let response = client.broadcast_tx_sync(tx_data).await.map_err(|e| {
            ErrorObjectOwned::owned(-32603, format!("Broadcast error: {}", e), None::<()>)
        })?;

        //Ok(format!("0x{}", hex::encode(response.hash)))
        Ok(format!("0x{:x}", eth_tx_hash))
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> jsonrpsee::core::RpcResult<Option<TransactionReceipt>> {
        let state = self.state.read().unwrap();
        //レシートの取得
        let receipt_key: Vec<u8> = [b"receipt:".as_slice(), tx_hash.as_slice()].concat();
        let Some(receipt) = state.get_receipt_struct(&receipt_key) else {
            return Ok(None);
        };

        //TxLookupの取得
        let tx_lookup_key: Vec<u8> = [b"tx_lookup:".as_slice(), tx_hash.as_slice()].concat();
        let Some((block_hash, tx_index)) = state.get_block_hash(&tx_lookup_key) else {
            return Ok(None);
        };

        //Blockの取得
        let Some(block) = state.get_full_block(&block_hash.as_slice()) else {
            return Ok(None);
        };

        let tx_index_usize = tx_index as usize;
        let Some(tx) = block.body.transactions.get(tx_index_usize) else {
            return Ok(None);
        };

        let status = match receipt.receipt.status {
            alloy_consensus::Eip658Value::Eip658(true) => Some(1),
            alloy_consensus::Eip658Value::Eip658(false) => Some(0),
            _ => None,
        };

        let gas_used = if tx_index == 0 {
            receipt.receipt.cumulative_gas_used
        } else {
            // 1つ前のTXを取得
            let prev_tx = &block.body.transactions[tx_index_usize - 1];
            // RLP化してハッシュを計算
            let mut prev_tx_rlp = Vec::new();
            prev_tx.encode(&mut prev_tx_rlp);
            let prev_tx_hash = alloy_primitives::keccak256(&prev_tx_rlp);
            // DBから1つ前のレシートを取得
            let prev_receipt_key = [b"receipt:".as_slice(), prev_tx_hash.as_slice()].concat();
            if let Some(prev_receipt) = state.get_receipt_struct(&prev_receipt_key) {
                // 差分を計算！
                receipt.receipt.cumulative_gas_used - prev_receipt.receipt.cumulative_gas_used
            } else {
                tracing::warn!("1つ前のレシートの取得に失敗しました");
                return Ok(None);
            }
        };
        let sender_opt = get_sender(&tx);
        let contract_address = if tx.t_to.is_create() {
            if let Some(sender) = sender_opt {
                Some(alloy_primitives::Address::create(
                    &sender,
                    tx.t_nonce as u64,
                ))
            } else {
                None
            }
        } else {
            None
        };

        // 内部ログを RPC 用のログ構造体にマッピング
        let rpc_logs: Vec<alloy_rpc_types::Log> = receipt
            .receipt
            .logs
            .iter()
            .enumerate()
            .map(|(i, eth_log)| alloy_rpc_types::Log {
                inner: eth_log.clone(),
                block_hash: Some(block_hash),
                block_number: Some(block.header.number),
                transaction_hash: Some(tx_hash),
                transaction_index: Some(tx_index),
                log_index: Some(i as u64),
                removed: false,
                block_timestamp: None,
            })
            .collect();

        // --- レシート特有のデータは `inner` 用の構造体にまとめる ---
        let inner_receipt = alloy_consensus::ReceiptWithBloom {
            receipt: alloy_consensus::Receipt {
                status: receipt.receipt.status,
                cumulative_gas_used: receipt.receipt.cumulative_gas_used,
                logs: rpc_logs,
            },
            logs_bloom: receipt.logs_bloom,
        };

        // --- 最終的な TransactionReceipt の組み立て ---
        let rpc_receipt = TransactionReceipt {
            transaction_hash: tx_hash,
            transaction_index: Some(tx_index),
            block_hash: Some(block_hash),
            block_number: Some(block.header.number),
            from: sender_opt.unwrap_or_default(),
            to: tx.t_to.into(),
            gas_used: gas_used as u64,
            contract_address,
            effective_gas_price: 0,
            blob_gas_used: None,
            blob_gas_price: None,
            inner: alloy_consensus::ReceiptEnvelope::Legacy(inner_receipt),
        };

        Ok(Some(rpc_receipt))
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        _block: Option<String>,
    ) -> jsonrpsee::core::RpcResult<String> {
        let mut state = self.state.write().unwrap();
        let nonce = if let Some(account) = state.contain_mpt(&address) {
            account.nonce
        } else {
            0
        };
        Ok(format!("0x{:x}", nonce))
    }

    async fn get_transaction_by_hash(
        &self,
        tx_hash: B256,
    ) -> jsonrpsee::core::RpcResult<Option<RPCTransaction>> {
        let state = self.state.read().unwrap();
        //TxLookupの取得
        let tx_lookup_key: Vec<u8> = [b"tx_lookup:".as_slice(), tx_hash.as_slice()].concat();
        let Some((block_hash, tx_index)) = state.get_block_hash(&tx_lookup_key) else {
            return Ok(None);
        };
        //Blockの取得
        let Some(block) = state.get_full_block(&block_hash[..]) else {
            return Ok(None);
        };
        //Transactionを取得
        let tx_index_usize = tx_index as usize;
        let Some(tx) = block.body.transactions.get(tx_index_usize) else {
            return Ok(None);
        };

        //送信者の復元
        let Some(sender_address) = get_sender(&tx) else {
            return Ok(None);
        };

        // 2. v値からパリティと Chain ID を復元 (EIP-155対応)
        let v: u64 = tx.t_w.try_into().unwrap_or(0);
        let (y_parity, chain_id) = if v == 27 || v == 28 {
            (v == 28, None)
        } else if v >= 35 {
            ((v - 35) % 2 != 0, Some((v - 35) / 2))
        } else {
            (false, None)
        };

        // 3. 署名オブジェクトの構築
        let signature = Signature::new(tx.t_r, tx.t_s, y_parity);

        // 4. TxLegacy (レガシートランザクション) の構築
        let tx_legacy = alloy_consensus::TxLegacy {
            chain_id,
            nonce: tx.t_nonce.try_into().unwrap_or(0),
            gas_price: tx.t_price.try_into().unwrap_or(0),
            gas_limit: tx.t_gas_limit.try_into().unwrap_or(0),
            to: tx.t_to.clone(),
            value: tx.t_value,
            input: tx.data.clone(),
        };

        // 5. Envelope に包む (TxLegacy -> Signed -> Recovered -> TxEnvelope)
        let signed_tx = alloy_consensus::Signed::new_unchecked(tx_legacy, signature, tx_hash);
        let tx_envelope = alloy_consensus::TxEnvelope::Legacy(signed_tx);
        let recovered_tx = Recovered::new_unchecked(tx_envelope, sender_address);

        // 6. 最終的な RPC用 Transaction 構造体の生成
        let rpc_tx = RPCTransaction {
            inner: recovered_tx,
            block_hash: Some(block_hash),
            block_number: Some(block.header.number),
            transaction_index: Some(tx_index),
            effective_gas_price: Some(tx.t_price.try_into().unwrap_or(0)),
            block_timestamp: Some(block.header.timestamp),
        };

        Ok(Some(rpc_tx))
    }

    async fn get_balance(
        &self,
        address: Address,
        index_string: Option<String>,
    ) -> jsonrpsee::core::RpcResult<String> {
        //blockからstate_rootを取り出す
        let state = self.state.read().unwrap();
        // index_string が None の場合は "latest" とみなす
        let block_param = index_string.unwrap_or_else(|| String::from("latest"));

        // パラメータを i64 の index に変換
        let index: i64 = if block_param == "latest" || block_param == "pending" {
            state.current_block_number()
        } else {
            // "0x" を取り除いて 16進数(base 16) として i64 にパースする
            let hex_str = block_param.trim_start_matches("0x");
            match i64::from_str_radix(hex_str, 16) {
                Ok(idx) => idx,
                Err(_) => {
                    tracing::warn!("無効なブロックパラメータです: {}", block_param);
                    return Ok(String::from("0x0")); // エラーの代わりに 0 ETH を返す
                }
            }
        };

        //Blockの取得
        let Some(block) = state.get_full_block_from_index(index) else {
            return Ok(format!("0x{:x}", U256::ZERO));
        };
        //state_rootを取得
        let target_root = block.header.state_root;
        let Some(target_balance) = state.get_balance_state(&address, target_root) else {
            return Ok(format!("0x{:x}", U256::ZERO));
        };

        return Ok(format!("0x{:x}", target_balance));
    }

    async fn get_block_by_number(
        &self,
        number: BlockNumberOrTag,
        full_transactions: bool,
    ) -> jsonrpsee::core::RpcResult<Option<RpcBlock>> {
        let state = self.state.read().unwrap();

        let block_number = match number {
            BlockNumberOrTag::Latest | BlockNumberOrTag::Pending => {
                state.current_block_number() as u64
            }
            BlockNumberOrTag::Number(n) => n,
            BlockNumberOrTag::Earliest => 0,
            _ => return Ok(None), // 他のタグ（safe, finalized）は今のところNone
        };
        //Blockの取得
        let Some(block) = state.get_full_block_from_index(block_number as i64) else {
            return Ok(None);
        };

        // 2. ブロックハッシュの計算 (ヘッダーを RLP 化して keccak256)
        let mut header_rlp = Vec::new();
        block.header.encode(&mut header_rlp);
        let calculated_block_hash = alloy_primitives::keccak256(&header_rlp);

        // 3. ブロック内トランザクションのハッシュ配列を事前計算
        let mut tx_hashes = Vec::new();
        for tx in &block.body.transactions {
            let mut tx_rlp = Vec::new();
            tx.encode(&mut tx_rlp);
            tx_hashes.push(alloy_primitives::keccak256(&tx_rlp));
        }

        // 4. `full_transactions` フラグに応じたトランザクションデータの分岐組み立て
        let transactions = if full_transactions {
            let mut rpc_txs = Vec::new();
            for (i, tx) in block.body.transactions.iter().enumerate() {
                let tx_hash = tx_hashes[i];
                let sender = get_sender(tx).unwrap_or_default();

                let v: u64 = tx.t_w.try_into().unwrap_or(0);
                let (y_parity, chain_id) = if v == 27 || v == 28 {
                    (v == 28, None)
                } else if v >= 35 {
                    ((v - 35) % 2 != 0, Some((v - 35) / 2))
                } else {
                    (false, None)
                };

                let signature = alloy_primitives::Signature::new(tx.t_r, tx.t_s, y_parity);
                let tx_legacy = alloy_consensus::TxLegacy {
                    chain_id,
                    nonce: tx.t_nonce.try_into().unwrap_or(0),
                    gas_price: tx.t_price.try_into().unwrap_or(0),
                    gas_limit: tx.t_gas_limit.try_into().unwrap_or(0),
                    to: tx.t_to.clone(),
                    value: tx.t_value,
                    input: tx.data.clone(),
                };

                let signed_tx =
                    alloy_consensus::Signed::new_unchecked(tx_legacy, signature, tx_hash);
                let tx_envelope = alloy_consensus::TxEnvelope::Legacy(signed_tx);
                let recovered_tx =
                    alloy_consensus::transaction::Recovered::new_unchecked(tx_envelope, sender);

                let rpc_tx = RPCTransaction {
                    inner: recovered_tx,
                    block_hash: Some(calculated_block_hash),
                    block_number: Some(block_number),
                    transaction_index: Some(i as u64),
                    effective_gas_price: Some(tx.t_price.try_into().unwrap_or(0)),
                    block_timestamp: Some(block.header.timestamp),
                };
                rpc_txs.push(rpc_tx);
            }
            BlockTransactions::Full(rpc_txs)
        } else {
            // フラグが false の場合はハッシュの配列だけを詰める
            BlockTransactions::Hashes(tx_hashes)
        };

        // 5. alloy_rpc_types::Header の組み立て
        let rpc_header = RpcHeader {
            hash: calculated_block_hash,
            inner: block.header,
            total_difficulty: Some(alloy_primitives::U256::ZERO),
            size: Some(alloy_primitives::U256::from(header_rlp.len())),
        };

        // 6. 最終的な RPC用フルブロックの返却
        let rpc_block = RpcBlock {
            header: rpc_header,
            transactions,
            uncles: vec![], // CometBFT/PoA運用のためアンクルブロックは常に空
            withdrawals: None,
        };

        Ok(Some(rpc_block))
    }

    async fn eth_call(
        &self,
        request: alloy_rpc_types::TransactionRequest,
        block_number: Option<BlockNumberOrTag>,
    ) -> jsonrpsee::core::RpcResult<String> {
        tracing::info!("[eth_call]が使われた!!!");

        //WorldStaeからRocksDBWrapper,
        let (db_wrapper, state_root) = {
            let state = self.state.read().unwrap(); // ロック取得

            let block_number = match block_number.unwrap_or(BlockNumberOrTag::Latest) {
                BlockNumberOrTag::Latest | BlockNumberOrTag::Pending => {
                    state.current_block_number() as u64
                }
                BlockNumberOrTag::Number(n) => n,
                BlockNumberOrTag::Earliest => 0,
                _ => {
                    return Err(ErrorObjectOwned::owned(
                        -32602,
                        "Unsupported block tag",
                        None::<()>,
                    ));
                }
            };
            //Blockの取得
            let Some(block) = state.get_full_block_from_index(block_number as i64) else {
                return Err(ErrorObjectOwned::owned(
                    -32603,
                    "Block not found",
                    None::<()>,
                ));
            };

            (state.data.clone(), block.header.state_root)
        };

        let mut tmp_state = WorldState::new_for_call(db_wrapper, state_root);

        // TransactionRequestからTransactionを作成
        let tx = Transaction {
            t_nonce: request.nonce.unwrap_or(0) as usize,
            t_price: U256::from(request.gas_price.unwrap_or(0)),
            t_gas_limit: U256::from(request.gas.unwrap_or(30_000_000)),
            t_to: request.to.unwrap_or(TxKind::Create),
            t_value: request.value.unwrap_or(U256::ZERO),
            data: request.input.into_input().unwrap_or_default(),
            t_w: U256::ZERO,
            t_r: U256::ZERO,
            t_s: U256::ZERO,
        };

        // BlockHeaderを作成
        let mut header = BlockHeader::default();

        // Transaction実行構造体LEVIATHANを作成
        let mut tmp_leviathan = LEVIATHAN::new(self.version);
        //LEVIATHAN構造体をeth_callモードに!!
        tmp_leviathan.eth_call = Some(request.from.unwrap_or_default());

        let _ = tmp_leviathan.execution(&mut tmp_state, tx, &header);

        // Transactionを実行
        let return_hex = format!("0x{}", hex::encode(&tmp_leviathan.return_data));

        tracing::info!("[eth_call]終了!!!");
        Ok(return_hex)
    }

    async fn get_logs(
        &self,
        filter: Filter,
    ) -> Result<Vec<alloy_rpc_types::Log>, ErrorObjectOwned> {
        let (block_vec, block_num_vec) = {
            let state = self.state.read().unwrap(); // ロック取得
            let latest_block_num = state.current_block_number() as u64;

            // 2. 検索開始ブロック (from_block) の決定
            let from_block = filter.get_from_block().unwrap_or(latest_block_num);

            // 3. 検索終了ブロック (to_block) の決定
            let to_block = filter.get_to_block().unwrap_or(latest_block_num);

            tracing::info!(
                "[eth_getLogs] Block {} から {} まで検索します",
                from_block,
                to_block
            );

            let mut block_vec = Vec::new();
            let mut block_num_vec = Vec::new();

            // ブロックを取得
            for block_num in from_block..=to_block {
                if let Some(block) = state.get_full_block_from_index(block_num as i64) {
                    let bloom = block.header.logs_bloom;
                    // ログに出力して確認してみる
                    tracing::debug!("Block {}: Bloom = {:?}", block_num, bloom);

                    if !is_bloom_match(&bloom, &filter) {
                        tracing::debug!("Block {} は bloom判定でfalse", block_num);
                        continue;
                    }
                    block_vec.push(block);
                    block_num_vec.push(block_num);
                } else {
                    tracing::warn!("Block {} がDBに見つかりませんでした", block_num);
                }
            }
            (block_vec, block_num_vec)
        };

        let mut result_logs = Vec::new();

        let state = self.state.read().unwrap(); // ロック取得
        // 該当したブロックをループ
        for (block, block_num) in block_vec.into_iter().zip(block_num_vec.into_iter()) {
            // ブロックの中の全トランザクションをループ
            for (tx_index, tx) in block.body.transactions.iter().enumerate() {
                let mut tx_rlp = Vec::new();
                tx.encode(&mut tx_rlp);
                let tx_hash = keccak256(tx_rlp);

                //レシートを取得
                if let Some(mut receipt_with_bloom_rlp) = state.get_receipt(&tx_hash.as_slice()) {
                    let Ok(receipt_with_bloom) =
                        ReceiptWithBloom::<Receipt>::decode(&mut receipt_with_bloom_rlp.as_slice())
                    else {
                        tracing::warn!("[eth_getLogs] レシートのデコードに失敗");
                        return Err(ErrorObjectOwned::owned(
                            -32602,
                            "無効なパラメータです",
                            None::<()>,
                        ));
                    };

                    //レシートレベルの Bloom チェック
                    if !is_bloom_match(&receipt_with_bloom.logs_bloom, &filter) {
                        continue;
                    }

                    tracing::debug!("TX {} が第3関門を突破", hex::encode(tx_hash));

                    //ログの厳密チェック (Exact Match)
                    for (log_index, log) in receipt_with_bloom.receipt.logs.iter().enumerate() {
                        if is_exact_match(log, &filter) {
                            tracing::info!("ログが完全に一致しました！");

                            // RPCで返すための Log 型に変換する
                            let rpc_log = format_to_rpc_log(
                                block_num,
                                block.header.hash_slow(), // ブロックハッシュ
                                tx_hash,
                                tx_index as u64,
                                log_index as u64,
                                log,
                            );
                            result_logs.push(rpc_log);
                        }
                    }
                }
            }
        }

        Ok(result_logs)
    }
}

pub async fn run_rpc_server(state: Arc<RwLock<WorldState>>, version: VersionId) {
    // 1. CORSの設定
    let cors = CorsLayer::permissive();

    // 2. ミドルウェアの構築
    let middleware = tower::ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    // サーバーのビルド (ポート8545)
    let server = ServerBuilder::default()
        .set_http_middleware(middleware)
        .build("0.0.0.0:8545")
        .await
        .expect("RPCサーバーの起動に失敗しました");

    // 実装インスタンスの作成とRPCモジュール化
    let rpc_impl = LeviathanRPC::new(state, version);
    let handle = server.start(rpc_impl.into_rpc());

    tracing::info!("JSON-RPCサーバーを 127.0.0.1:8545 で起動しました");

    // サーバーが終了しないように待機
    handle.stopped().await;
}
