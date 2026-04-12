use alloy_primitives::{Address, Bytes, U256};
use serde::Deserialize;
use std::collections::HashMap;

/// テストファイルのルート。
/// JSONの最上位は `"add0": { ... }` のようになっているため、HashMapで受け取ります。
pub type VmTestSuite = HashMap<String, VmTest>;

/// 1つのテストケース全体
#[derive(Debug, Deserialize)]
pub struct VmTest {
    // _info は不要なので定義から外すことで自動的に無視されます
    pub env: Env,
    pub exec: Exec,
    pub pre: HashMap<Address, AccountState>,
    pub post: Option<HashMap<Address, AccountState>>, // テストによっては無い場合もあるのでOption
    pub gas: Option<U256>,                            // 期待される残りガス
    pub out: Option<Bytes>,                           // 期待されるリターンデータ
}

/// ブロック環境 (ExecutionEnvironmentの元データ)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")] // currentCoinbase などをスネークケースに自動変換
pub struct Env {
    pub current_coinbase: Address,
    pub current_difficulty: U256,
    pub current_gas_limit: U256,
    pub current_number: U256,
    pub current_timestamp: U256,
}

/// 実行コンテキスト (ExecutionEnvironmentの元データ)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Exec {
    pub address: Address,
    pub caller: Address,
    pub code: Bytes, // "0x..." の文字列を自動でVec<u8>として扱える型です
    pub data: Bytes,
    pub gas: U256,
    pub gas_price: U256,
    pub origin: Address,
    pub value: U256,
}

/// 実行前・実行後のアカウント状態 (WorldStateの元データ)
#[derive(Debug, Deserialize)]
pub struct AccountState {
    pub balance: U256,
    pub code: Bytes,
    pub nonce: U256,
    pub storage: HashMap<U256, U256>, // "0x00": "0xff..fe" を U256 のペアとして自動変換
}
