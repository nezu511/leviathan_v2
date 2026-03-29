// src/test/state_parser.rs
use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct StateTestSuite {
    #[serde(flatten)]
    pub tests: HashMap<String, StateTestCase>,
}

#[derive(Deserialize, Debug)]
pub struct StateTestCase {
    pub env: EnvData,
    pub pre: HashMap<String, AccountData>,
    pub transaction: TransactionData,
    pub expect: Vec<ExpectData>,
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
    pub to: String,
    pub value: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct ExpectData {
    pub network: Vec<String>,
    pub result: HashMap<String, AccountData>,
}
