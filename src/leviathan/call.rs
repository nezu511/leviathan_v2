#![allow(dead_code)]

use crate::evm::evm::EVM;
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{
    BackupSubstate, BlockHeader, ExecutionEnvironment, Log, SubState, Transaction, VersionId,
};
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::evm_trait::{Gfunction, Hfunction, Ofunction, Xi, Zfunction};
use crate::my_trait::leviathan_trait::{
    CompiledContract, MessageCall, RoleBack, State, TransactionExecution,
};
use alloy_primitives::{I256, U256};
use rlp::RlpStream;
use sha3::{Digest, Keccak256};
use std::collections::HashMap;

impl MessageCall for LEVIATHAN {
    fn message_call(
        &mut self,
        state: &mut WorldState,
        substate: &mut SubState,
        sender: Address,
        origin: Address,
        recipient: Address,
        contract: Address,
        gas: U256,
        price: U256,
        eth: U256,
        apparent_value: U256,
        data: Vec<u8>,
        depth: usize,
        sudo: bool,
        block_header: &BlockHeader,
    ) -> Result<(U256, Vec<u8>, Option<Address>), (U256, Option<Vec<u8>>, Option<Address>)> {
        if !substate.a_access.contains(&recipient) {
            substate.a_access.push(recipient.clone())
        }
        self.substate_backup = BackupSubstate::backup(substate); //サブステートのバックアップ

        //残高の移動
        if eth != U256::ZERO {
            if state.is_empty(&sender) {
                return Err((gas, None, None));
            }
            if state.is_empty(&recipient) {
                if !state.is_physically_exist(&recipient) {
                    state.add_account(&recipient, Account::new()); //アカウントを追加
                    Action::Account_creation(recipient.clone()).push(self, state); //アカウントが存在しない場合
                }
            }
            if sender != recipient {
                Action::Send_eth(sender.clone(), recipient.clone(), eth).push(self, state); //ロールバック用
                state.send_eth(&sender, &recipient, eth); //残高の移動
            }
        } else if self.version < VersionId::SpuriousDragon {
            //Ethereumの初期はvalue=0であっても無条件でアカウントを作成
            if state.is_empty(&recipient) {
                if !state.is_physically_exist(&recipient) {
                    state.add_account(&recipient, Account::new()); //アカウントを追加
                    Action::Account_creation(recipient.clone()).push(self, state); //アカウントが存在しない場合
                }
            }
        }

        //Execution Environmentの構築
        let mut execution_environment = Box::new(ExecutionEnvironment::new(
            recipient.clone(),
            origin.clone(),
            price,
            data,
            sender.clone(),
            apparent_value,
            Vec::new(),
            block_header,
            depth,
            sudo,
        ));

        //プリコンパイル判定と実行の要件
        let contract_u256 = contract.to_u256();
        let result = match contract_u256 {
            val if val == U256::from(1) => {
                //ECDSA公開鍵復元
                LEVIATHAN::ecrec(gas, &execution_environment.i_data)
            }
            val if val == U256::from(2) => {
                //SHA256
                LEVIATHAN::sha256(gas, &execution_environment.i_data)
            }

            val if val == U256::from(3) => {
                //RIP160
                LEVIATHAN::precompile_ripemd160(gas, &execution_environment.i_data)
            }

            val if val == U256::from(4) => {
                //ID: 入力データをそのまま返す
                LEVIATHAN::precompile_identity(gas, &execution_environment.i_data)
            }

            val if val == U256::from(5) => todo!(), //EXPMOD
            val if val == U256::from(6) => todo!(), //BN_ADD
            val if val == U256::from(7) => todo!(), //BN_MUL
            val if val == U256::from(8) => todo!(), //SNARKV
            val if val == U256::from(9) => todo!(), //BLAKE2_F

            _ => {
                //通常のスマートコントラクト呼び出し

                let exe_code = state.get_code(&contract).unwrap_or(Vec::new());
                execution_environment.i_byte = exe_code;
                //仮想マシンの実行
                let mut evm = Box::new(EVM::new(&execution_environment, self.version.clone()));
                evm.gas = gas;
                let result = evm.evm_run(self, state, substate, &mut execution_environment);
                match result {
                    Ok(output) => {
                        let rest_gas = evm.return_gas();
                        Ok((rest_gas, output))
                    }

                    Err(Some(revert_data)) => {
                        let rest_gas = evm.return_gas();
                        Err((rest_gas, Some(revert_data)))
                    }

                    Err(None) => Err((U256::ZERO, None)),
                }
            }
        };
        match result {
            Ok((return_gas, output)) => {
                //最終処理
                return Ok((return_gas, output, None));
            }

            Err((revert_gas, Some(revert_data))) => {
                //REVERT
                tracing::info!("[MessageCall] Revert");
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                return Err((revert_gas, Some(revert_data), None));
            }

            Err((gas, None)) => {
                //Z関数による停止
                tracing::info!("[MessageCall] 例外停止");
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                return Err((U256::ZERO, None, None));
            }
        }
    }
}
