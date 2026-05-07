use alloy_primitives::{hex, B256, Address, TxKind};
use alloy_rlp::{Decodable, Encodable, Header};
use alloy_consensus::{Block, BlockBody, Header as BlockHeader, Receipt, ReceiptWithBloom};
use alloy_rpc_types::TransactionReceipt;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::ServerBuilder;
use jsonrpsee::types::ErrorObjectOwned;
use std::sync::Arc;
use std::sync::RwLock;
use tendermint_rpc::{Client, HttpClient};
use secp256k1::{
    Message, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use sha3::{Digest, Keccak256};
use bytes::BytesMut;

use leviathan_v2::leviathan::world_state::WorldState;
use leviathan_v2::leviathan::structs::Transaction;

#[rpc(server)]
pub trait EthApi {
    #[method(name = "eth_chainId")]
    async fn chain_id(&self) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_blockNumber")]
    async fn block_number(&self) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_sendRawTransaction")]
    async fn send_raw_transaction(&self, tx_bytes: String) -> jsonrpsee::core::RpcResult<String>;

    #[method(name = "eth_getTransactionReceipt")]
    async fn get_transaction_receipt(&self, tx_hash: B256) -> jsonrpsee::core::RpcResult<Option<TransactionReceipt>>;

   // #[method(name = "eth_getTransactionByHash")]
   // async fn get_transaction_by_hash(&self, tx_hash: B256) -> jsonrpsee::core::RpcResult<Option<Transaction>>;
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

    async fn get_transaction_receipt(&self, tx_hash: B256) -> jsonrpsee::core::RpcResult<Option<TransactionReceipt>> {
        let state = self.state.read().unwrap();
        //レシートの取得
        let receipt_key: Vec<u8> = [b"receipt:".as_slice(), tx_hash.as_slice()].concat();
        let Some(receipt) = state.get_receipt_struct(&receipt_key) else {
            return Ok(None)
        };

        //TxLookupの取得
        let tx_lookup_key: Vec<u8> = [b"tx_lookup:".as_slice(), tx_hash.as_slice()].concat();
        let Some((block_hash, tx_index)) = state.get_block_hash(&tx_lookup_key) else {
            return Ok(None)
        };

        //Blockの取得
        let Some(block) = state.get_full_block(&block_hash.as_slice()) else {
            return Ok(None)
        };

        let tx_index_usize = tx_index as usize;
        let Some(tx) = block.body.transactions.get(tx_index_usize) else {
            return Ok(None)
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
                Some(alloy_primitives::Address::create(&sender, tx.t_nonce as u64))
            } else {
                None
            }
        } else {
            None
        };

        // 内部ログを RPC 用のログ構造体にマッピング
        let rpc_logs: Vec<alloy_rpc_types::Log> = receipt.receipt.logs.iter().enumerate().map(|(i, eth_log)| {
            alloy_rpc_types::Log {
                address: eth_log.address,
                topics: eth_log.data.topics().to_vec(),
                data: eth_log.data.data.clone(),
                block_hash: Some(block_hash),
                block_number: Some(block.header.number),
                transaction_hash: Some(tx_hash),
                transaction_index: Some(tx_index),
                log_index: Some(i as u64),
                removed: false,
            }
        }).collect();

        //レシート特有のデータは `inner` 用の構造体にまとめる
        let inner_receipt = alloy_consensus::ReceiptWithBloom {
            receipt: alloy_consensus::Receipt {
                status: receipt.receipt.status,
                cumulative_gas_used: receipt.receipt.cumulative_gas_used,
                logs: rpc_logs,
            },
            logs_bloom: receipt.logs_bloom,
        };

        //最終的な TransactionReceipt の組み立て
        let rpc_receipt = TransactionReceipt {
            transaction_hash: tx_hash,
            transaction_index: Some(tx_index),
            block_hash: Some(block_hash),
            block_number: Some(block.header.number),
            from: sender_opt.unwrap_or_default(),
            to: tx.t_to.into(), // TxKind から Option<Address> へ自動変換
            gas_used: gas_used as u64, // Optionではなく直接u64
            contract_address,
            effective_gas_price: 0,
            inner: inner_receipt.into(), 
            ..Default::default()
        };

        Ok(Some(rpc_receipt))
    }
}



pub fn get_sender(transaction: &Transaction) -> Option<Address> {
    let Ok(t_w_u64) = u64::try_from(transaction.t_w) else {
        tracing::warn!("t_w is too large for u64");
        return None
    };
    let (recovery_id_u8, chain_id) = if t_w_u64 == 27 || t_w_u64 == 28 {
        ((t_w_u64 - 27) as u8, None)
    } else if t_w_u64 >= 35 {
        (((t_w_u64 - 35) % 2) as u8, Some((t_w_u64 - 35) / 2))
    } else {
        tracing::warn!("[get_sender] Invalid v value");
        return None
    };
    let Ok(recovery_id) = RecoveryId::try_from(recovery_id_u8 as i32) else {
        tracing::warn!("Invalid recovery id");
        return None
    };
    // 1. 各要素のRLPペイロード長を事前計算する (alloy-rlpの特徴)
    let mut payload_length = 0;
    payload_length += transaction.t_nonce.length();
    payload_length += transaction.t_price.length();
    payload_length += transaction.t_gas_limit.length();

    let to_slice = match &transaction.t_to {
        TxKind::Call(address) => address.0.as_slice(),
        TxKind::Create => &[], // 空のバイト列
    };
    payload_length += to_slice.length();
    payload_length += transaction.t_value.length();
    payload_length += transaction.data.length();
    //EIP-155用に3フィールドを準備
    if let Some(cid) = chain_id {
        payload_length += cid.length();
        payload_length += 0u64.length();
        payload_length += 0u64.length();
    }
    // 2. バッファを確保し、リストのヘッダーを書き込む
    let mut out = BytesMut::with_capacity(payload_length + 10); // ヘッダー分少し余分に確保
    Header {
        list: true,
        payload_length,
    }
    .encode(&mut out);
    transaction.t_nonce.encode(&mut out);
    transaction.t_price.encode(&mut out);
    transaction.t_gas_limit.encode(&mut out);
    to_slice.encode(&mut out);
    transaction.t_value.encode(&mut out);
    transaction.data.encode(&mut out);

    if let Some(cid) = chain_id {
        cid.encode(&mut out);
        0u64.encode(&mut out);
        0u64.encode(&mut out);
    }
    let rlp_encoded = out.freeze();
    //4. Keccak256でハッシュ化して32バイトのh(T)を得る
    let mut hasher = Keccak256::new();
    hasher.update(&rlp_encoded);
    let tx_hash_bytes: [u8; 32] = hasher.finalize().into();
    // --- 公開鍵のリカバリ部分 ---
    let message = Message::from_digest(tx_hash_bytes);
    // 【解決策6】 `to_big_endian` の代わりに `to_be_bytes::<32>()` を使う
    let mut sig_bytes = [0u8; 64];
    sig_bytes[0..32].copy_from_slice(&transaction.t_r.to_be_bytes::<32>());
    sig_bytes[32..64].copy_from_slice(&transaction.t_s.to_be_bytes::<32>());
    let Ok(signature) = RecoverableSignature::from_compact(&sig_bytes, recovery_id) else {
        tracing::warn!("Invalid signature");
        return None
    };
    let secp = Secp256k1::verification_only();
    // 【解決策7】 最新版では `&message` ではなく `message` (値渡し) にする
    let Ok(public_key) = secp.recover_ecdsa(message, &signature) else {
        tracing::warn!("Failed to recover public key");
        return None
    };
    // あとは前回のコードと同じようにアドレスを抽出！
    let uncompressed_pubkey = public_key.serialize_uncompressed();
    let pubkey_hash = Keccak256::digest(&uncompressed_pubkey[1..65]);
    let mut sender_address = [0u8; 20];
    sender_address.copy_from_slice(&pubkey_hash[12..32]);
    let sender_address = Address::new(sender_address);
    return Some(sender_address)
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
