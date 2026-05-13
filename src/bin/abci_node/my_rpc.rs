use alloy_consensus::{Block, BlockBody, Header as BlockHeader, Receipt, ReceiptWithBloom, TxEnvelope, Signed};
use alloy_consensus::transaction::Recovered;
use alloy_primitives::{Address, B256, TxKind, hex, Signature}; 
use alloy_rlp::{Decodable, Encodable, Header};
use alloy_rpc_types::{TransactionReceipt, Transaction as RPCTransaction};
use bytes::BytesMut;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::ServerBuilder;
use jsonrpsee::types::ErrorObjectOwned;
use std::sync::Arc;
use secp256k1::{
    Message, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use sha3::{Digest, Keccak256};
use std::sync::RwLock;
use tendermint_rpc::{Client, HttpClient};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use leviathan_v2::leviathan::structs::Transaction;
use leviathan_v2::leviathan::world_state::WorldState;
use crate::utils::get_sender;

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
    async fn get_transaction_by_hash(&self, tx_hash: B256) -> jsonrpsee::core::RpcResult<Option<RPCTransaction>>;
}

pub struct LeviathanRPC {
    state: Arc<RwLock<WorldState>>,
}

impl LeviathanRPC {
    pub fn new(state: Arc<RwLock<WorldState>>) -> Self {
        Self { state }
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

    async fn get_transaction_by_hash(&self, tx_hash: B256) -> jsonrpsee::core::RpcResult<Option<RPCTransaction>> {
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
        let signature = Signature::new(
            tx.t_r,
            tx.t_s,
            y_parity,
        );

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
        let rpc_tx = RPCTransaction{
            inner: recovered_tx,
            block_hash: Some(block_hash),
            block_number: Some(block.header.number),
            transaction_index: Some(tx_index),
            effective_gas_price: Some(tx.t_price.try_into().unwrap_or(0)),
            block_timestamp: Some(block.header.timestamp),
        };

        Ok(Some(rpc_tx))
    }
        
        

}


pub async fn run_rpc_server(state: Arc<RwLock<WorldState>>) {
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
    let rpc_impl = LeviathanRPC::new(state);
    let handle = server.start(rpc_impl.into_rpc());

    tracing::info!("JSON-RPCサーバーを 127.0.0.1:8545 で起動しました");

    // サーバーが終了しないように待機
    handle.stopped().await;
}
