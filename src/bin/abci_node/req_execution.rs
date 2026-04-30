use crate::LeviathanApp;
use alloy_primitives::{Address, TxKind, U256, hex};
use alloy_rlp::{Decodable, RlpDecodable, RlpEncodable};
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::{BlockHeader, Transaction, VersionId};
use leviathan_v2::leviathan::world_state::WorldState;
use leviathan_v2::my_trait::leviathan_trait::{State, TransactionExecution};
use tendermint_proto::abci::{
    Event, EventAttribute, ExecTxResult, RequestCheckTx, RequestFinalizeBlock, RequestInfo,
    ResponseCheckTx, ResponseCommit, ResponseFinalizeBlock, ResponseInfo,
};

pub trait PI {
    fn tx_execution(&self, req: &RequestFinalizeBlock) -> Vec<ExecTxResult>;
}

impl PI for LeviathanApp {
    fn tx_execution(&self, req: &RequestFinalizeBlock) -> Vec<ExecTxResult> {
        //ブロックヘッダーの作成
        let h_number = U256::from(req.height);
        let timestamp_seconds = req.time.unwrap_or_default().seconds;
        let h_timestamp = U256::from(timestamp_seconds);
        let h_beneficiary = Address::from_slice(&req.proposer_address);

        let block_header = BlockHeader {
            h_beneficiary,
            h_timestamp,
            h_number,
            h_prevrandao: U256::ZERO,           // ダミー（PoS等の乱数用）
            h_gaslimit: U256::from(30_000_000), // ブロックのガスリミット
            h_basefee: U256::ZERO,              // EIP-1559のベースフィー
        };

        tracing::info!(
            "[FINALIZE_BLOCK] Height: {}, Time: {}, Txs: {}",
            h_number,
            h_timestamp,
            req.txs.len()
        );

        //トランザクションを実行
        let mut state = self.state.lock().unwrap();
        let mut leviathan = self.leviathan.lock().unwrap();
        let mut tx_results = Vec::new();

        for tx in &req.txs {
            let mut raw_tx_slice = tx.as_ref();

            match Transaction::decode(&mut raw_tx_slice) {
                Ok(transaction) => {
                    tracing::info!(
                        "[CHECK_TX] デコード成功: Nonce={}, GasLimit={}",
                        transaction.t_nonce,
                        transaction.t_gas_limit
                    );

                    //実行
                    let gas_wanted =
                        u64::try_from(transaction.t_gas_limit).unwrap_or(u64::MAX) as i64;
                    let result = leviathan.execution(&mut state, transaction, &block_header);
                    match result {
                        Ok((final_bill_gas, logs)) => {
                            let mut abci_events = Vec::new();
                            for eth_log in logs {
                                let mut attributes = Vec::new();

                                // 1. アドレスを属性に追加
                                attributes.push(EventAttribute {
                                    key: "address".to_string(),
                                    value: format!("0x{}", hex::encode(eth_log.address.0)),
                                    index: true,
                                });

                                // 2. トピックを属性に追加
                                for (i, topic) in eth_log.topic.iter().enumerate() {
                                    attributes.push(EventAttribute {
                                        key: format!("topic{}", i),
                                        value: format!(
                                            "0x{}",
                                            hex::encode(topic.to_be_bytes::<32>())
                                        ),
                                        index: true,
                                    });
                                }

                                // 3. データを属性に追加
                                attributes.push(EventAttribute {
                                    key: "data".to_string(),
                                    value: format!("0x{}", hex::encode(&eth_log.data)),
                                    index: false,
                                });

                                // 1つの Ethereum Log を 1つの CometBFT Event にまとめる
                                abci_events.push(Event {
                                    r#type: "evm_log".to_string(), // イベントの種類を識別する名前
                                    attributes,
                                });
                            }

                            tx_results.push(ExecTxResult {
                                code: 0,
                                log: "Success".to_string(),
                                events: abci_events,
                                gas_wanted,
                                gas_used: u64::try_from(final_bill_gas).unwrap_or(u64::MAX) as i64,
                                ..Default::default()
                            });
                        }

                        Err((final_bill_gas, logs)) => {
                            tx_results.push(ExecTxResult {
                                code: 1,
                                log: "Execution Failed".to_string(),
                                gas_wanted,
                                gas_used: u64::try_from(final_bill_gas).unwrap_or(u64::MAX) as i64,
                                ..Default::default()
                            });
                        }
                    }
                }
                Err(err) => {
                    // デコード失敗（スパムやEthereum互換ではないフォーマット）
                    tracing::warn!("[CHECK_TX] RLPデコード失敗: {:?}", err);

                    // codeを非ゼロ（例: 1）にしてCometBFTに弾かせる
                    tx_results.push(ExecTxResult {
                        code: 1,
                        log: format!("Decode Error: {}", err),
                        ..Default::default()
                    });
                }
            }
        }
        tx_results
    }
}
