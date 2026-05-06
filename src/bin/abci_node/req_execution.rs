use crate::LeviathanApp;
use alloy_primitives::{Address, U256, hex, keccak256, Bloom};
use alloy_consensus::{Header as BlockHeader, Block, Receipt, ReceiptWithBloom};
use alloy_rlp::{Decodable, Encodable};
use eth_trie::{EthTrie, MemoryDB, Trie};
use std::sync::Arc;

use leviathan_v2::leviathan::structs::{Transaction};
use leviathan_v2::my_trait::leviathan_trait::TransactionExecution;
use tendermint_proto::abci::{
    Event, EventAttribute, ExecTxResult, RequestFinalizeBlock,
};

pub trait PI {
    fn tx_execution(&self, req: &RequestFinalizeBlock) -> Vec<ExecTxResult>;
}

impl PI for LeviathanApp {
    fn tx_execution(&self, req: &RequestFinalizeBlock) -> Vec<ExecTxResult> {
        //ブロックヘッダーの作成
        let timestamp_seconds = req.time.unwrap_or_default().seconds;
        let h_beneficiary = Address::from_slice(&req.proposer_address);

        let mut block_header = BlockHeader {
            beneficiary: h_beneficiary,
            timestamp: timestamp_seconds as u64,
            number: req.height as u64,
            gas_limit: 30_000_000, // ブロックのガスリミット
            ..Default::default()
        };

        tracing::info!(
            "[FINALIZE_BLOCK] Height: {}, Time: {}, Txs: {}",
            req.height as u64,
            timestamp_seconds as u64,
            req.txs.len()
        );

        //トランザクションを実行
        let mut state = self.state.write().unwrap();

        //ブロックナンバーをWorldStateに書き込む
        state.update_block_number(req.height);

        let mut leviathan = self.leviathan.lock().unwrap();
        let mut tx_results = Vec::new();
        let mut cumulative_gas:u64 = 0;
        let mut block_bloom = Bloom::default();

        //トランザクション・レシートのルートハッシュ算出用のMPTを準備
        let memdb = Arc::new(MemoryDB::new(true));
        let mut eth_trie = EthTrie::new(memdb.clone());
        let origin_root= eth_trie.root_hash().unwrap();
        let mut transaction_trie =
            EthTrie::from(memdb.clone(), origin_root).unwrap();
        let mut receipt_trie =
            EthTrie::from(memdb.clone(), origin_root).unwrap();

        for (i, tx) in req.txs.iter().enumerate() {
            let mut raw_tx_slice = tx.as_ref();
            let mut transaction_rlp = raw_tx_slice.clone();

            match Transaction::decode(&mut raw_tx_slice) {
                Ok(transaction) => {

                    let mut mpt_key = Vec::new();
                    i.encode(&mut mpt_key);
                    //トランザクションををMPTに入れる
                    transaction_trie.insert(&mpt_key, &transaction_rlp).unwrap();


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
                            let Ok(final_bill_gas_u64) = u64::try_from(final_bill_gas) else {
                                panic!("U256の値が大きすぎて u64 に収まりません！");
                            };
                            //累積ガスを更新
                            cumulative_gas += final_bill_gas_u64;
                            //レシートを作成
                            let receipt = Receipt {
                                status: alloy_consensus::Eip658Value::Eip658(true),
                                cumulative_gas_used: cumulative_gas,
                                logs: logs.clone(),
                            };
                            let receipt_with_bloom = ReceiptWithBloom::from(receipt);
                            //ブロック用のbloomを算出
                            block_bloom |= receipt_with_bloom.logs_bloom;

                            //レシートをRLP化
                            let mut rlp_receipt = Vec::new();
                            receipt_with_bloom.encode(&mut rlp_receipt);

                            let receipt_hash = keccak256(&rlp_receipt);
                            let receipt_key: Vec<u8> = [b"receipt:".as_slice(), receipt_hash.as_slice()].concat();
                            //RocksDBWrapperに保存
                            state.insert_receipt(&receipt_key, &rlp_receipt);

                            //レシートをMPTに入れる
                            receipt_trie.insert(&mpt_key, &rlp_receipt).unwrap();

                            let mut abci_events = Vec::new();
                            for eth_log in logs {
                                let mut attributes = Vec::new();

                                // 1. アドレスを属性に追加
                                attributes.push(EventAttribute {
                                    key: "address".to_string(),
                                    value: format!("0x{}", hex::encode(eth_log.address.0)),
                                    index: true,
                                });

                                for (i, topic) in eth_log.data.topics().iter().enumerate() { // ★ eth_log.data.topics() に変更
                                    attributes.push(EventAttribute {
                                        key: format!("topic{}", i),
                                        // ★ topic はすでに B256 なので、そのまま as_slice() でバイト列として扱える
                                        value: format!("0x{}", hex::encode(topic.as_slice())),
                                        index: true,
                                    });
                                }

                                // 3. データを属性に追加
                                attributes.push(EventAttribute {
                                    key: "data".to_string(),
                                    // ★ eth_log.data (LogData構造体) の中の .data (Bytes型) にアクセスする
                                    value: format!("0x{}", hex::encode(&eth_log.data.data)),
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

                        Err((final_bill_gas, _logs)) => {
                            let Ok(final_bill_gas_u64) = u64::try_from(final_bill_gas) else {
                                panic!("U256の値が大きすぎて u64 に収まりません！");
                            };
                            //累積ガスを更新
                            cumulative_gas += final_bill_gas_u64;
                            //レシートを作成
                            let receipt = Receipt {
                                status: alloy_consensus::Eip658Value::Eip658(true),
                                cumulative_gas_used: cumulative_gas,
                                logs: _logs,
                            };
                            let receipt_with_bloom = ReceiptWithBloom::from(receipt);
                            //ブロック用のbloomを算出
                            block_bloom |= receipt_with_bloom.logs_bloom;

                            //レシートをRLP化
                            let mut rlp_receipt = Vec::new();
                            receipt_with_bloom.encode(&mut rlp_receipt);

                            let receipt_hash = keccak256(&rlp_receipt);
                            let receipt_key: Vec<u8> = [b"receipt:".as_slice(), receipt_hash.as_slice()].concat();
                            //RocksDBWrapperに保存
                            state.insert_receipt(&receipt_key, &rlp_receipt);

                            //レシートをMPTに入れる
                            receipt_trie.insert(&mpt_key, &rlp_receipt).unwrap();

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
        //ブロックヘッダーを完成させる
        //レシートMPTのルートハッシュ
        block_header.receipts_root = receipt_trie.root_hash().unwrap();
        //トランザクションMPTのルートハッシュを求める
        block_header.transactions_root = transaction_trie.root_hash().unwrap();
        //MPTルートハッシュ
        block_header.state_root = state.eth_trie.root_hash().unwrap();
        //消費ガスの累計
        block_header.gas_used = cumulative_gas;
        //全レシートのログからのbloom
        block_header.logs_bloom = block_bloom;
        //親ブロックのハッシュ
        block_header.parent_hash = state.parent_block;


        tx_results
    }
}
