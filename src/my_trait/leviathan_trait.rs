use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction, BlockHeader};
use alloy_primitives::{I256, U256};

pub trait State {
    fn is_empty(&self,address: &Address) -> bool;   //空だとtrue;
                                            
    fn get_balance(&self, address: &Address) -> Option<U256>;

    fn get_code(&self, address: &Address) -> Option<Vec<u8>>;

    fn get_storage_value(&self, address: &Address, key: &U256) -> Option<U256>;

    fn get_nonce(&self, address: &Address) -> Option<u32>;
    
    // 書き込み系
    fn set_balance(&mut self, address: &Address, value:U256);

    fn inc_nonce(&mut self, address: &Address);

    fn dec_nonce(&mut self, address: &Address);

    fn set_storage(&mut self, address: &Address, key: U256, value: U256);

    fn set_code(&mut self, address: &Address, code: Vec<u8>);
    
    fn remove_storage(&mut self, address: &Address, key:U256) ;

    fn send_eth(&mut self, from: &Address, to: &Address, eth:U256) -> Result<(),&'static str>;

    fn buy_gas(&mut self, address: &Address, limit: U256, price: U256) -> Result<U256,&'static str>;

    fn reset_storage(&mut self, address: &Address);

    fn delete_account(&mut self, address: &Address);

    fn add_account(&mut self, address: &Address, account: Account);

}

pub trait TransactionChecks {
     fn transaction_checks(state: &mut WorldState, transaction:&Transaction, inti_gas: &U256, pre_cost: &U256, block_header: &BlockHeader) -> Result<Address,&'static str>;
}


pub trait TransactionExecution {
     fn execution(&self, state: &mut WorldState, transaction:Transaction, block_header: &BlockHeader) -> Result<(U256, Vec<Log>, bool),(U256, Vec<Log>, bool)>;
}

pub trait ContractCreation {
    fn contract_creation(&mut self,
                         state: &mut WorldState,
                         substate: &mut SubState,
                         sender: Address,   //送信者のアドレス
                         origin: Address,   //Originalアドレス
                         gas: U256,      //利用可能なガス
                         price: U256,      //ガス価格
                         eth: U256,      //送るETH
                         init_code: Vec<u8>,   //EVM初期化バイトコード
                         depth: usize,       //コールスタック深さ
                         salt: Option<U256>,      //Creat2用のソルト
                         sudo: bool,       //ステートへの変更権限
                         block_header: &BlockHeader,
                         ) -> Result<(U256,Vec<u8>),(U256,Vec<u8>)>;     //ガスとデータ？
}
                         

pub trait MessageCall {
    fn message_call(&mut self,
                    state: &mut WorldState,
                    substate: &mut SubState,
                    sender: Address,   //送信者のアドレス
                    origin: Address,   //Originalアドレス
                    recipient: Address,   //送金を受け取るアドレス
                    contract: Address,   //EVMコードを読み出して実行するアドレス
                    gas: U256,      //利用可能なガス
                    price: U256,      //ガス価格
                    eth: U256,      //送るETH
                    apparent_value: U256,      //見かけ上送るETH
                    data: Vec<u8>,   //データ
                    depth: usize,       //コールスタック深さ
                    sudo: bool       //ステートへの変更権限
                         ) -> Result<(U256,Vec<u8>),(U256,Vec<u8>)>;     //ガスとデータ？
}

pub trait RoleBack {
    fn roleback(&mut self, state: &mut WorldState) -> Result<(), &'static str>;
}


