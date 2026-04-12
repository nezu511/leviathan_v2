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
    CompiledContract, MessageCall, RoleBack, State,
};
use alloy_primitives::U256;
use sha3::Digest;

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
            if state.is_empty(&recipient)
                && !state.is_physically_exist(&recipient) {
                    state.add_account(&recipient, Account::new()); //アカウントを追加
                    Action::AccountCreation(recipient.clone()).push(self, state); //アカウントが存在しない場合
                }
            if sender != recipient {
                Action::SendEth(sender.clone(), recipient.clone(), eth).push(self, state); //ロールバック用
                state.send_eth(&sender, &recipient, eth); //残高の移動
            }
        } else if self.version < VersionId::SpuriousDragon {
            //Ethereumの初期はvalue=0であっても無条件でアカウントを作成
            if state.is_empty(&recipient)
                && !state.is_physically_exist(&recipient) {
                    state.add_account(&recipient, Account::new()); //アカウントを追加
                    Action::AccountCreation(recipient.clone()).push(self, state); //アカウントが存在しない場合
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

            val if val == U256::from(5) => {
                //EXPMOD
                LEVIATHAN::expmod(gas, &execution_environment.i_data)
            }

            val if val == U256::from(6) => {
                //BN_ADD
                LEVIATHAN::bn_add(gas, &execution_environment.i_data)
            }

            val if val == U256::from(7) => {
                //BN_MUL
                LEVIATHAN::bn_mul(gas, &execution_environment.i_data)
            }

            val if val == U256::from(8) => {
                //SNARKV
                LEVIATHAN::bn_pairing(gas, &execution_environment.i_data)
            }

            val if val == U256::from(9) => todo!(), //BLAKE2_F

            _ => {
                //通常のスマートコントラクト呼び出し

                let exe_code = state.get_code(&contract).unwrap_or_default();
                execution_environment.i_byte = exe_code;
                //仮想マシンの実行
                let mut evm = Box::new(EVM::new(&execution_environment, self.version));
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
                Ok((return_gas, output, None))
            }

            Err((revert_gas, Some(revert_data))) => {
                //REVERT
                tracing::info!("[MessageCall] Revert");
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                Err((revert_gas, Some(revert_data), None))
            }

            Err((_gas, None)) => {
                //Z関数による停止
                tracing::info!("[MessageCall] 例外停止");
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone()); //SubStateの巻き戻し
                Err((U256::ZERO, None, None))
            }
        }
    }
}
