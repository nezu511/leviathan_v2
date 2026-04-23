#![allow(dead_code)]

use crate::evm::evm::EVM;
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{
    BackupSubstate, BlockHeader, ExecutionEnvironment, SubState, VersionId,
};
use crate::leviathan::world_state::{Account, WorldState};
use crate::my_trait::evm_trait::{Gfunction, Ofunction, Xi};
use crate::my_trait::leviathan_trait::{CompiledContract, MCC, MessageCall, RoleBack, State};
use alloy_primitives::{Address, U256};
use sha3::Digest;

//cpu実行時間を記録するため
use alloy_primitives::hex;
#[cfg(test)]
use std::fs::OpenOptions;
#[cfg(test)]
use std::io::Write;
#[cfg(test)]
use std::time::Instant;

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
        //サブステートのa_touchに追加
        if !substate.a_touch.contains(&recipient) {
            substate.a_touch.push(recipient.clone())
        }

        //残高の移動
        if eth != U256::ZERO {
            if state.is_dead(self.version, &sender) {
                return Err((gas, None, None));
            }
            if state.is_dead(self.version, &recipient) && !state.is_physically_exist(&recipient) {
                state.add_account(&recipient, Account::new()); //アカウントを追加
                Action::AccountCreation(recipient.clone()).push(self, state); //アカウントが存在しない場合
            }
            if sender != recipient {
                Action::SendEth(sender.clone(), recipient.clone(), eth).push(self, state); //ロールバック用
                state.send_eth(&sender, &recipient, eth); //残高の移動
            }
        } else if self.version < VersionId::SpuriousDragon {
            //Ethereumの初期はvalue=0であっても無条件でアカウントを作成
            if state.is_dead(self.version, &recipient) && !state.is_physically_exist(&recipient) {
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
        let contract_u256 = U256::from_be_bytes(contract.into_word().0);

        /*/ ★ 1. 計測開始 (テスト時のみコンパイル)
        #[cfg(test)]
        let (is_precompile, start_time) = {
            let is_pre = contract_u256 >= U256::from(1) && contract_u256 <= U256::from(9);
            let start = if is_pre { Some(Instant::now()) } else { None };
            (is_pre, start)
        };
        */

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
                if self.version >= VersionId::Byzantium {
                    LEVIATHAN::expmod(gas, &execution_environment.i_data, self.version)
                } else {
                    tracing::info!("expmodがフォークに対応していないため実行できない");
                    Ok((gas, Vec::new()))
                }
            }

            val if val == U256::from(6) => {
                //BN_ADD
                if self.version >= VersionId::Byzantium {
                    LEVIATHAN::bn_add(gas, &execution_environment.i_data, self.version)
                } else {
                    tracing::info!("bn_addがフォークに対応していないため実行できない");
                    Ok((gas, Vec::new()))
                }
            }

            val if val == U256::from(7) => {
                //BN_MUL
                if self.version >= VersionId::Byzantium {
                    LEVIATHAN::bn_mul(gas, &execution_environment.i_data, self.version)
                } else {
                    tracing::info!("bn_mulがフォークに対応していないため実行できない");
                    Ok((gas, Vec::new()))
                }
            }

            val if val == U256::from(8) => {
                //SNARKV
                if self.version >= VersionId::Byzantium {
                    LEVIATHAN::bn_pairing(gas, &execution_environment.i_data, self.version)
                } else {
                    tracing::info!("bn_pairingがフォークに対応していないため実行できない");
                    Ok((gas, Vec::new()))
                }
            }

            val if val == U256::from(9) => {
                //BLAKE2_F
                if self.version >= VersionId::Istanbul {
                    todo!()
                } else {
                    todo!()
                    //Ok((gas, Vec::new()))
                }
            }

            val if val == U256::from(10) => {
                //my_rsa
                LEVIATHAN::my_rsa(gas, &execution_environment.i_data, self.version)
            }

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

        /*/ ★ 2. CSVへの記録処理 (テスト時のみコンパイル)
        #[cfg(test)]
                {
                    if let Some(start) = start_time {
                        let elapsed_micros = start.elapsed().as_micros();
                        // データの「中身」ではなく「長さ」だけ取得
                        let input_len = execution_environment.i_data.len();

                        let (status, consumed_gas) = match &result {
                            Ok((rest_gas, _)) => {
                                ("Success", gas.saturating_sub(*rest_gas))
                            }
                            Err((rest_gas, _)) => {
                                ("Revert_or_Error", gas.saturating_sub(*rest_gas))
                            }
                        };

                        // InputData(Hex) を InputLen に差し替え
                        let csv_line = format!(
                            "{},{},{},{},{}\n",
                            contract_u256, input_len, consumed_gas, status, elapsed_micros
                            );

                        if let Ok(mut file) = OpenOptions::new()
                            .create(true)
                                .append(true)
                                .open("gas_analy/stRevertTest_benchmarks.csv")
                                {
                                    let _ = file.write_all(csv_line.as_bytes());
                                }
                    }
                }
                */
        match result {
            Ok((return_gas, output)) => {
                //最終処理
                Ok((return_gas, output, None))
            }

            Err((revert_gas, Some(revert_data))) => {
                //REVERT
                tracing::info!("[MessageCall] Revert");
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone(), self.version); //SubStateの巻き戻し
                Err((revert_gas, Some(revert_data), None))
            }

            Err((_gas, None)) => {
                //Z関数による停止
                tracing::info!("[MessageCall] 例外停止");
                self.roleback(state); //Roleback実行
                substate.road_backup(self.substate_backup.clone(), self.version); //SubStateの巻き戻し
                Err((U256::ZERO, None, None))
            }
        }
    }
}
