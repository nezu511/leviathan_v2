#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution, TransactionChecks, ContractCreation};
use crate::my_trait::evm_trait::{Xi, Gfunction, Zfunction, Hfunction, Ofunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction, BlockHeader};
use crate::evm::evm::EVM;
use sha3::{Keccak256, Digest};
use rlp::RlpStream;


impl ContractCreation for LEVIATHAN {
    fn contract_creation(&mut self, state: &mut WorldState, substate: &mut SubState, sender: Address, origin: Address,
                         gas: U256, price: U256, eth: U256, init_code: Vec<u8>, depth: usize, salt: Option<U256>, sudo: bool, block_header: &BlockHeader
                         ) -> Result<(U256,Vec<u8>),(U256,Vec<u8>)> {

        //新しいアカウントのアドレス
        let byte: Vec<u8> = if salt.is_none() {
            //CREATE
            let nonce = state.get_nonce(&sender).unwrap() -1;
            let mut stream = RlpStream::new_list(2);
            stream.append(&sender.0.as_ref());
            stream.append(&nonce);
            stream.out().to_vec()
        }else{
            //CREATE2
            let mut tmp = [0u8;85];
            tmp[0] = 0xff;      //定数
            tmp[1..21].copy_from_slice(&sender.0);  //送信者のアドレス
            let salt_byte:[u8;32] = salt.unwrap().to_be_bytes(); 
            tmp[21..53].copy_from_slice(&salt_byte);   //salt
            let mut hasher = Keccak256::new();
            hasher.update(&init_code);
            let result:[u8;32] = hasher.finalize().try_into().unwrap();
            tmp[53..85].copy_from_slice(&result);
            tmp.to_vec()
        };
        let mut hasher = Keccak256::new();
        hasher.update(&byte);
        let result:[u8;32] = hasher.finalize().try_into().unwrap();
        let mut tmp = [0u8;20];
        tmp.copy_from_slice(&result[12..32]);
        let contract_address = Address::new(tmp);
        
        //サブステートのアクセス済みアカウントに追加
        if !substate.a_access.contains(&contract_address) {
            substate.a_access.push(contract_address.clone())
        }

        //Nonceを1にする．
        state.inc_nonce(&contract_address);
        //送金する
        state.send_eth(&sender, &contract_address, eth);
        //storageRootを空にする
        state.reset_storage(&contract_address);
        //codehashに空配列をセット
        state.set_code(&contract_address, Vec::<u8>::new());

        //Execution Environmentの構築
        let mut execution_environment = ExecutionEnvironment::new(contract_address.clone(),origin.clone(), price, Vec::new(), sender.clone(),eth, 
                                                            init_code, block_header, depth,sudo);

        //仮想マシンの実行
        let mut evm = EVM::new(&execution_environment);
        let result = evm.evm_run(state, substate, &mut execution_environment);
        //Ok()：正常停止
        //Err(None) => Z関数による停止
        //Err(Some(Vec<u8>)) => REVERTによる停止

        match result {
            Ok(output) => (),

            Err(Some(revert_data)) => {
                //REVERT
            },

            Err(None) => {
                //Z関数による停止
            }
        }



        return Ok((U256::ZERO, Vec::new()));


    }
}
