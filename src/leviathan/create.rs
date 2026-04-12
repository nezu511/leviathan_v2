#![allow(dead_code)]

use crate::evm::evm::EVM;
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{
    BackupSubstate, BlockHeader, ExecutionEnvironment, SubState, VersionId,
};
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::evm_trait::{Gfunction, Ofunction, Xi};
use crate::my_trait::leviathan_trait::{
    ContractCreation, RoleBack, State,
};
use alloy_primitives::{U256, hex};
use rlp::RlpStream;
use sha3::{Digest, Keccak256};
use std::collections::HashMap;

impl ContractCreation for LEVIATHAN {
    fn contract_creation(
        &mut self,
        state: &mut WorldState,
        substate: &mut SubState,
        sender: Address,
        origin: Address,
        gas: U256,
        price: U256,
        eth: U256,
        init_code: Vec<u8>,
        depth: usize,
        salt: Option<U256>,
        sudo: bool,
        block_header: &BlockHeader,
    ) -> Result<(U256, Vec<u8>, Option<Address>), (U256, Option<Vec<u8>>, Option<Address>)> {
        //新しいアカウントのアドレス
        let byte: Vec<u8> = if salt.is_none() {
            //CREATE
            let nonce = state.get_nonce(&sender).unwrap() - 1;
            let mut stream = RlpStream::new_list(2);
            stream.append(&sender.0.as_ref());
            stream.append(&nonce);
            stream.out().to_vec()
        } else {
            //CREATE2
            let mut tmp = [0u8; 85];
            tmp[0] = 0xff; //定数
            tmp[1..21].copy_from_slice(&sender.0); //送信者のアドレス
            let salt_byte: [u8; 32] = salt.unwrap().to_be_bytes();
            tmp[21..53].copy_from_slice(&salt_byte); //salt
            let mut hasher = Keccak256::new();
            hasher.update(&init_code);
            let result: [u8; 32] = hasher.finalize().into();
            tmp[53..85].copy_from_slice(&result);
            tmp.to_vec()
        };
        let mut hasher = Keccak256::new();
        hasher.update(&byte);
        let result: [u8; 32] = hasher.finalize().into();
        let mut tmp = [0u8; 20];
        tmp.copy_from_slice(&result[12..32]);
        let contract_address = Address::new(tmp);
        tracing::info!("【ContractCreation】:0x{}", hex::encode(contract_address.0)); //アドレス

        //事前チェックはcollisonのみ
        let nonce = state.get_nonce(&contract_address).unwrap_or(0);
        let code = state
            .get_code(&contract_address)
            .unwrap_or_default();
        let is_collision =
            nonce != 0 || !code.is_empty() || !state.is_storage_empty(&contract_address); // アドレス衝突
        if is_collision {
            tracing::warn!("[ContractCreaion]　collisionによる例外停止");
            return Err((U256::ZERO, None, None));
        }

        //サブステートのアクセス済みアカウントに追加
        if !substate.a_access.contains(&contract_address) {
            substate.a_access.push(contract_address.clone())
        }
        self.substate_backup = BackupSubstate::backup(substate); //サブステートのバックアップ

        //Nonceを1にする．
        if state.is_empty(&contract_address)
            && !state.is_physically_exist(&contract_address) {
                state.add_account(&contract_address, Account::new()); //アカウントを追加
                Action::AccountCreation(contract_address.clone()).push(self, state); //アカウントが存在しない場合
            }
        if self.version >= VersionId::SpuriousDragon {
            Action::AddNonce(contract_address.clone()).push(self, state); //ロールバック用
            state.inc_nonce(&contract_address);
        }
        //送金する
        if state.is_empty(&sender) {
            return Err((gas, None, None));
        }
        if state.is_empty(&contract_address)
            && !state.is_physically_exist(&contract_address) {
                state.add_account(&contract_address, Account::new()); //アカウントを追加
                Action::AccountCreation(contract_address.clone()).push(self, state); //アカウントが存在しない場合
            }
        Action::SendEth(sender.clone(), contract_address.clone(), eth).push(self, state); //ロールバック用
        state.send_eth(&sender, &contract_address, eth);
        //storageRootを空にする
        Action::Reset_storage(contract_address.clone(), HashMap::<U256, U256>::new())
            .push(self, state); //ロールバック用
        state.reset_storage(&contract_address);
        //codehashに空配列をセット
        Action::StoreCode(contract_address.clone(), Vec::new()).push(self, state); //ロールバック用
        state.set_code(&contract_address, Vec::<u8>::new());

        //Execution Environmentの構築
        let mut execution_environment = Box::new(ExecutionEnvironment::new(
            contract_address.clone(),
            origin.clone(),
            price,
            Vec::new(),
            sender.clone(),
            eth,
            init_code,
            block_header,
            depth,
            sudo,
        ));

        //仮想マシンの実行
        let mut evm = Box::new(EVM::new(&execution_environment, self.version));
        evm.gas = gas;
        let result = evm.evm_run(self, state, substate, &mut execution_environment);
        //Ok()：正常停止
        //Err(None) => Z関数による停止
        //Err(Some(Vec<u8>)) => REVERTによる停止

        match result {
            Ok(output) => {
                //コードデプロイ費用
                let deposit_gas = 200 * output.len();
                let rest_gas = evm.return_gas();
                tracing::debug!(
                deposit_gas = %deposit_gas,
                rest_gas = %rest_gas,
                );
                let mut return_gas = rest_gas;
                if self.version >= VersionId::Homestead {
                    if U256::from(deposit_gas) > rest_gas {
                        tracing::info!("[ContractCreation] 例外停止:コードデプロイ費用不足");
                        self.roleback(state); //Roleback実行
                        substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                        return Err((U256::ZERO, None, None));
                    }
                    //コードのサイズ制限
                    if self.version >= VersionId::SpuriousDragon
                        && output.len() > 24576 {
                            tracing::info!("[ContractCreation] 例外停止:コードサイズ制限");
                            self.roleback(state); //Roleback実行
                            substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                            return Err((U256::ZERO, None, None));
                        }
                    //不正なプレフィックス
                    if self.version >= VersionId::London
                        && !output.is_empty() && output[0] == 0xefu8 {
                            tracing::info!("[ContractCreation] 例外停止:不正なプレフィックス");
                            self.roleback(state); //Roleback実行
                            substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                            return Err((U256::ZERO, None, None));
                        }
                    return_gas -= U256::from(deposit_gas);
                    state.set_code(&contract_address, output);
                } else if U256::from(deposit_gas) < rest_gas {
                    return_gas -= U256::from(deposit_gas);
                    state.set_code(&contract_address, output);
                }

                Ok((return_gas, Vec::<u8>::new(), Some(contract_address.clone())))
            }

            Err(Some(revert_data)) => {
                //REVERT
                tracing::info!("[ContractCreation] Revert");
                let revert_gas = evm.return_gas(); //ガス返却
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                Err((revert_gas, Some(revert_data), None))
            }

            Err(None) => {
                //Z関数による停止
                tracing::info!("[ContractCreation] 例外停止");
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                Err((U256::ZERO, None, None))
            }
        }
    }
}
