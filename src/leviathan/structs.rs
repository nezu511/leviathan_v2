#![allow(dead_code)]

use crate::leviathan::world_state::{WorldState, Address, Account};
use alloy_primitives::{I256, U256};
use std::collections::HashMap;

pub struct Transaction {
    pub t_nonce: usize,
    pub t_gas_limit: U256,
    pub t_price: U256,
    pub t_to: Option<Address>,
    pub t_value: U256,
    pub data: Vec<u8>,
    pub t_r: U256,
    pub t_s: U256,
    pub t_w: U256
}


pub struct SubState {
    pub a_des: Vec<Address>,    //破棄されるアカウント集合
    pub a_log: Vec<Log>,        //ログリスト
    pub a_touch: Vec<Address>,  //さわられたアカウントリスト：最後にEmptyのアカウントは消す
    pub a_reimburse: i64,      //ガスの払い戻し
    pub a_access: Vec<Address>, //アクセスされたアカウントリスト：２回目移行のアクセスはガス代割引
    pub a_access_storage: HashMap<Address, HashMap<U256, U256>>  //一度アクセスしたストレージのスロット
}

impl SubState {
    pub fn new()  -> Self{
        let a_des  = Vec::<Address>::new();
        let a_log = Vec::<Log>::new();
        let a_touch = Vec::<Address>::new();
        let a_reimburse = 0i64;
        let a_access = Vec::<Address>::new();
        let a_access_storage = HashMap::new();
        Self {a_des, a_log, a_touch, a_reimburse, a_access, a_access_storage}
    }
}

pub struct Log {
    address: Address,
    topic: Vec<U256>, //0~4個
    data: Vec<u8>,
}

impl Log {
    pub fn new(address:Address, topic:Vec<U256>, data:Vec<u8>) -> Self {
        Self{address, topic, data}
    }
}

pub struct ExecutionEnvironment <'a> {
    pub i_address: Address,     //現在実行中のコードを所有しているアカウント
    pub i_origin: Address,      //実行の起点となった大本のトランザクション送信者
    pub i_gas_price: U256,      //この実行の起点となったトランザクションの署名者が支払うガス価格
    pub i_data: Vec<u8>,        //実行への入力データ
    pub i_sender: Address,      //このコードを実行する直接の原因となったアカウント
    pub i_value: U256,          //実行に伴ってアカウントに渡される総金額
    pub i_byte: Vec<u8>,        //実行されるマシンコードのバイト列
    pub i_block_header: &'a BlockHeader,    //現在のブロックヘッダー情報
    pub i_depth: usize,
    pub i_permission: bool,     //ステートを変更する権限の有無
}

pub struct BlockHeader {
    pub h_beneficiary: Address,     //ブロックの優先手数料を受け取るアドレス
    pub h_timestamp: U256,          //ブロック生成時の妥当なUnixスタンプ:
    pub h_number: U256,             //ブロックnumber
    pub h_prevrandao: U256,         //前のブロックbいー今ステートから提供される乱数生成用の値
    pub h_gaslimit: U256,           //ブロック全体のガス上限
    pub h_basefee: U256,            //消費されたガス１単位あたりにバーンされるお金
}
