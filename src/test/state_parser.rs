// src/test/state_parser.rs
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct StateTestSuite {
    #[serde(flatten)]
    pub tests: HashMap<String, StateTestCase>,
}

#[derive(Deserialize, Debug)]
pub struct StateTestCase {
    pub _info: Option<serde_json::Value>,
    pub env: EnvData,                           
    pub pre: HashMap<String, AccountData>,      
    pub transaction: TransactionData,
    pub post: HashMap<String, Vec<PostState>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EnvData {
    pub current_coinbase: String,
    pub current_difficulty: String,
    pub current_gas_limit: String,
    pub current_number: String,
    pub current_timestamp: String,
}

#[derive(Deserialize, Debug)]
pub struct AccountData {
    pub balance: Option<String>,
    pub code: Option<String>,
    pub nonce: Option<String>,
    pub storage: Option<HashMap<String, String>>,
    pub shouldnotexist: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionData {
    pub data: Vec<String>,
    pub gas_limit: Vec<String>,
    pub gas_price: String,
    pub nonce: String,
    pub secret_key: String,
    #[serde(default)]
    pub to: String,
    pub value: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct ExpectData {
    pub network: Vec<String>,
    pub result: HashMap<String, AccountData>,
    pub indexes: Option<TestIndexes>,
}

#[derive(Debug, Deserialize)]
pub struct PostState {
    pub hash: String,
    pub indexes: TestIndexes,                   
    pub logs: String,
}

// ▼▼▼ `state_parser.rs` の一番下の部分をこれに置き換える ▼▼▼

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum IndexType {
    Single(i32),     // "data": 0 のような単一の数字
    Multi(Vec<i32>), // "data": [0, 2] のような配列
}

impl IndexType {
    // どちらの型が来ても、とりあえず最初の1つ目の数字を取り出す便利関数
    pub fn first(&self) -> i32 {
        match self {
            IndexType::Single(v) => *v,
            IndexType::Multi(v) => v.first().copied().unwrap_or(0),
        }
    }
}

// 万が一省略された時のためのデフォルト値
fn default_index() -> IndexType {
    IndexType::Single(0)
}

#[derive(Deserialize, Debug)]
pub struct TestIndexes {
    #[serde(default = "default_index")]
    pub data: IndexType,
    #[serde(default = "default_index")]
    pub gas: IndexType,
    #[serde(default = "default_index")]
    pub value: IndexType,
}
