#![allow(dead_code)]

use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{
    BackupSubstate, BlockHeader, Log, SubState, Transaction, VersionId,
};
use crate::leviathan::world_state::{Account, MptAccount, WorldState};
use crate::my_trait::leviathan_trait::{
    ContractCreation, MessageCall, State, TransactionChecks, TransactionExecution,
};
use alloy_primitives::{Address, U256, hex, keccak256};
use alloy_rlp::Encodable;
use eth_trie::{EthTrie, Trie};
use sha3::Digest;

pub struct LEVIATHAN {
    pub journal: Vec<Action>,
    pub substate_backup: BackupSubstate,
    pub version: VersionId,
}

impl LEVIATHAN {
    pub fn new(version: VersionId) -> Self {
        Self {
            journal: Vec::<Action>::new(),
            substate_backup: BackupSubstate::new(),
            version,
        }
    }

    pub fn merge(&mut self, mut children: LEVIATHAN) {
        self.journal.append(&mut children.journal);
    }
}

impl TransactionExecution for LEVIATHAN {
    fn execution(
        &mut self,
        state: &mut WorldState,
        transaction: Transaction,
        block_header: &BlockHeader,
    ) -> Result<(U256, Vec<Log>), (U256, Vec<Log>)> {
        tracing::info!("version: {:?}", self.version);
        //=======ステップ1===========
        //【初期ガスの計算】
        let base_gas = U256::from(21000); //基本料金
        let mut data_gas = U256::ZERO;
        let mut index = 0;

        //データに関するガス
        if self.version < VersionId::Istanbul {
            //Istanbul以前
            while index < transaction.data.len() {
                if transaction.data[index] == 0 {
                    data_gas = data_gas.saturating_add(U256::from(4));
                } else {
                    data_gas = data_gas.saturating_add(U256::from(68));
                }
                index += 1;
            }
        } else {
            while index < transaction.data.len() {
                if transaction.data[index] == 0 {
                    data_gas = data_gas.saturating_add(U256::from(4));
                } else {
                    data_gas = data_gas.saturating_add(U256::from(16));
                }
                index += 1;
            }
        }

        let mut contract_gas = U256::ZERO;
        if transaction.t_to.is_none() {
            //コントラクト作成追加費
            if self.version >= VersionId::Homestead {
                //Homestead以降
                contract_gas = contract_gas.saturating_add(U256::from(32000));

                if self.version >= VersionId::Shanghai {
                    //Shanghai以降
                    //Initcodeのサイズに対する従量課金
                    let words = U256::from(transaction.data.len()).saturating_add(U256::from(31))
                        / U256::from(32);
                    let word_gas = words.saturating_mul(U256::from(2));
                    contract_gas = contract_gas.saturating_add(word_gas);
                }
            }
        }
        let all_gas = base_gas + data_gas + contract_gas;
        //【事前支払いコスト】
        let max_cost =
            transaction.t_gas_limit.saturating_mul(transaction.t_price) + transaction.t_value;
        //【トランザクションの事前検証】
        let sender_address =
            self.transaction_checks(state, &transaction, &all_gas, &max_cost, block_header);
        if sender_address.is_err() {
            tracing::warn!("{}", sender_address.unwrap_err());
            return Err((U256::ZERO, Vec::new()));
        }
        let sender_address = sender_address.unwrap();

        //=======ステップ2===========
        //【Nonceの加算】
        if state.is_dead(self.version, &sender_address) {
            return Err((U256::ZERO, Vec::new())); //sender_addressが見つからないのは異常
        }
        state.inc_nonce(&sender_address);
        //【前払いガス代の徴収】
        let gas = state.buy_gas(
            &sender_address,
            transaction.t_gas_limit,
            transaction.t_price,
        );
        //ここからロールバックの起点:ロールバックが起きたらこの状態にする
        let mut substate = SubState::new();

        //a_touchにトランザクションの基本要素（送信者，ブロックの受取人）を追加
        substate.a_touch.push(sender_address.clone());
        substate.a_touch.push(block_header.h_beneficiary.clone());

        //gasから初期ガスを引く
        let mut gas = gas.unwrap();
        gas = gas.saturating_sub(all_gas);

        //=======ステップ3===========
        let result = if transaction.t_to.is_none() {
            //デバック出力
            tracing::info!(
            sender_address =  format_args!("0x{}", hex::encode(sender_address.0)),
            data = %hex::encode(&transaction.data),
            gas = %gas,
            gas_price = %transaction.t_price,
            send_eth = %transaction.t_value,
            "Transaction [CREATE]"
            );
            self.contract_creation(
                state,
                &mut substate,
                sender_address.clone(),
                sender_address.clone(),
                gas,
                transaction.t_price,
                transaction.t_value,
                transaction.data,
                0,
                None,
                true,
                block_header,
            )
        } else {
            let to_address = transaction.t_to.unwrap();
            //a_touchにトランザクションの基本要素（宛先）を追加
            substate.a_touch.push(to_address.clone());
            //デバック出力
            tracing::info!(
            sender_address =  format_args!("0x{}", hex::encode(sender_address.0)),
            to_address =  format_args!("0x{}", hex::encode(to_address.0)),
            data = %hex::encode(&transaction.data),
            gas = %gas,
            gas_price = %transaction.t_price,
            send_eth = %transaction.t_value,
            "Transaction [CALL]"
            );
            self.message_call(
                state,
                &mut substate,
                sender_address.clone(),
                sender_address.clone(),
                to_address.clone(),
                to_address.clone(),
                gas,
                transaction.t_price,
                transaction.t_value,
                transaction.t_value,
                transaction.data,
                0,
                true,
                block_header,
            )
        };

        //払い戻しガス
        match result {
            Ok((gas, _, _)) => {
                let used_gas = transaction.t_gas_limit.saturating_sub(gas);
                let max_refund = if self.version < VersionId::London {
                    //返金の上限がフォークで異なる
                    used_gas / U256::from(2)
                } else {
                    used_gas / U256::from(5)
                };
                let reimburse_u256 = U256::from(substate.a_reimburse.max(0) as u64);
                let reimburse = std::cmp::min(max_refund, reimburse_u256);
                let return_gas = gas.saturating_add(reimburse);
                //送信者への返金
                let reimburse = return_gas.saturating_mul(transaction.t_price);
                if state.is_dead(self.version, &sender_address) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&sender_address) {
                        state.add_account(&sender_address, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&sender_address, reimburse);
                //マイナーへの支払い
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(return_gas);
                let f = if self.version < VersionId::London {
                    transaction.t_price
                } else {
                    transaction.t_price - block_header.h_basefee
                };
                let reward = final_billed_gas.saturating_mul(f);
                if state.is_dead(self.version, &block_header.h_beneficiary) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&block_header.h_beneficiary) {
                        state.add_account(&block_header.h_beneficiary, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&block_header.h_beneficiary, reward);
                //デバック用
                tracing::info!(
                    beneficiary =  format_args!("0x{}", hex::encode(block_header.h_beneficiary.0)),
                    reward = %reward,
                    reimburse = %reimburse,
                    final_billed_gas = %final_billed_gas,
                    "[マイナーへの支払い]",
                );
                //substate.a_touchの処理
                while let Some(address) = substate.a_touch.pop() {
                    if state.is_dead(self.version, &address) {
                        let address_hash = keccak256(address);
                        state.eth_trie.remove(address_hash.as_slice());
                        state.cache.remove(&address);
                        tracing::info!(
                            address = format_args!("0x{}", hex::encode(address.0)),
                            "[a_touchによる削除]"
                        );
                    }
                }
                //substate.a_desの処理
                while let Some(address) = substate.a_des.pop() {
                    let address_hash = keccak256(address);
                    state.eth_trie.remove(address_hash.as_slice());
                    state.cache.remove(&address);
                    tracing::info!(
                        address = format_args!("0x{}", hex::encode(address.0)),
                        "[a_desによる削除]"
                    );
                }
                //MPT更新
                for (address, cache_account) in state.cache.iter() {
                    let mut storage_trie =
                        EthTrie::from(state.data.clone(), cache_account.storage_hash).unwrap();
                    let mut storage_changed = false;

                    //storageの値を書き込む
                    for (key, value) in cache_account.storage.iter() {
                        let key_byte: [u8; 32] = key.to_be_bytes();
                        let key_hash = keccak256(key_byte);
                        let existing_val_opt =
                            storage_trie.get(key_hash.as_slice()).unwrap_or(None);

                        if value.is_zero() {
                            if existing_val_opt.is_some() {
                                storage_trie.remove(key_hash.as_slice());
                                storage_changed = true;
                            }
                        } else {
                            let val_rlp_bytes = alloy_rlp::encode(value);
                            if existing_val_opt != Some(val_rlp_bytes.clone()) {
                                storage_trie
                                    .insert(key_hash.as_slice(), val_rlp_bytes.as_slice())
                                    .unwrap();
                                storage_changed = true;
                            }
                        }
                    }
                    //新しいstorage_rootを取得
                    let storage_root = if storage_changed {
                        storage_trie.root_hash().unwrap()
                    } else {
                        cache_account.storage_hash
                    };
                    //コードハッシュを取得
                    let code_hash = keccak256(cache_account.code.clone());
                    state
                        .code_storage
                        .entry(code_hash)
                        .or_insert(cache_account.code.clone());
                    tracing::info!(
                    address =  format_args!("0x{}", hex::encode(address.0)),
                    nonce = %cache_account.nonce,
                    balance = %cache_account.balance,
                    storage_root = format_args!("{}", storage_root),
                    code_hash = format_args!("{}", code_hash)
                    );
                    //mpt_accout作成
                    let mpt_account = MptAccount::new(
                        cache_account.nonce,
                        cache_account.balance,
                        storage_root,
                        code_hash,
                    );
                    //MPTに書き込む
                    let address_hash = keccak256(address);
                    let mut mpt_account_rlp_bytes = Vec::new();
                    mpt_account.encode(&mut mpt_account_rlp_bytes);

                    //MPTに現在登録されているRLPを取得
                    let existing_mpt_val =
                        state.eth_trie.get(address_hash.as_slice()).unwrap_or(None);

                    // 更新すべきか判定
                    let should_insert = match existing_mpt_val {
                        None => true, // MPTに存在しない（新規アカウント）なら絶対に挿入
                        Some(old_rlp) => {
                            // MPTに存在するなら、RLPの中身が変化している場合のみ挿入
                            old_rlp != mpt_account_rlp_bytes
                        }
                    };

                    // 更新
                    if should_insert {
                        tracing::debug!("更新: 0x{}", hex::encode(address));
                        let _ = state.eth_trie.remove(address_hash.as_slice());
                        state
                            .eth_trie
                            .insert(address_hash.as_slice(), mpt_account_rlp_bytes.as_slice())
                            .unwrap();
                    }
                }
                //eth_trieのルートハッシュを取得
                let new_state_root = state.eth_trie.root_hash().unwrap();
                state.update_eth_trie(new_state_root);

                Ok((final_billed_gas, substate.a_log.clone()))
            }
            Err((gas, _, _)) => {
                //送信者への返金
                let reimburse = gas.saturating_mul(transaction.t_price);
                if state.is_dead(self.version, &sender_address) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&sender_address) {
                        state.add_account(&sender_address, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&sender_address, reimburse);
                //マイナーへの支払い
                let final_billed_gas = transaction.t_gas_limit.saturating_sub(gas);
                let f = if self.version < VersionId::London {
                    transaction.t_price
                } else {
                    transaction.t_price - block_header.h_basefee
                };
                let reward = final_billed_gas.saturating_mul(f);
                if state.is_dead(self.version, &block_header.h_beneficiary) {
                    //add_balance前の確認
                    if !state.is_physically_exist(&block_header.h_beneficiary) {
                        state.add_account(&block_header.h_beneficiary, Account::new()); //アカウントを追加
                    }
                }
                state.add_balance(&block_header.h_beneficiary, reward);
                //デバック用
                tracing::info!(
                    beneficiary =  format_args!("0x{}", hex::encode(block_header.h_beneficiary.0)),
                    reward = %reward,
                    reimburse = %reimburse,
                    final_billed_gas = %final_billed_gas,
                    "[Err:マイナーへの支払い]",
                );
                //substate.a_touchの処理
                tracing::debug!("{:?}", substate.a_touch);
                while let Some(address) = substate.a_touch.pop() {
                    if state.is_dead(self.version, &address) {
                        let address_hash = keccak256(address);
                        state.eth_trie.remove(address_hash.as_slice());
                        state.cache.remove(&address);
                        tracing::info!(
                            address = format_args!("0x{}", hex::encode(address.0)),
                            "[a_touchによる削除]"
                        );
                    }
                }
                //substate.a_desの処理
                while let Some(address) = substate.a_des.pop() {
                    let address_hash = keccak256(address);
                    state.eth_trie.remove(address_hash.as_slice());
                    state.cache.remove(&address);
                    tracing::info!(
                        address = format_args!("0x{}", hex::encode(address.0)),
                        "[a_desによる削除]"
                    );
                }
                //MPT更新
                for (address, cache_account) in state.cache.iter() {
                    let mut storage_trie =
                        EthTrie::from(state.data.clone(), cache_account.storage_hash).unwrap();
                    let mut storage_changed = false;
                    //storageの値を書き込む
                    for (key, value) in cache_account.storage.iter() {
                        let key_byte: [u8; 32] = key.to_be_bytes();
                        let key_hash = keccak256(key_byte);
                        let existing_val_opt =
                            storage_trie.get(key_hash.as_slice()).unwrap_or(None);

                        if value.is_zero() {
                            if existing_val_opt.is_some() {
                                storage_trie.remove(key_hash.as_slice());
                                storage_changed = true;
                            }
                        } else {
                            let val_rlp_bytes = alloy_rlp::encode(value);
                            if existing_val_opt != Some(val_rlp_bytes.clone()) {
                                storage_trie
                                    .insert(key_hash.as_slice(), val_rlp_bytes.as_slice())
                                    .unwrap();
                                storage_changed = true;
                            }
                        }
                    }
                    //新しいstorage_rootを取得
                    let storage_root = if storage_changed {
                        storage_trie.root_hash().unwrap()
                    } else {
                        cache_account.storage_hash
                    };
                    //コードハッシュを取得
                    let code_hash = keccak256(cache_account.code.clone());
                    state
                        .code_storage
                        .entry(code_hash)
                        .or_insert(cache_account.code.clone());
                    let mpt_account = MptAccount::new(
                        cache_account.nonce,
                        cache_account.balance,
                        storage_root,
                        code_hash,
                    );
                    //MPTに書き込む
                    let address_hash = keccak256(address);
                    let mut mpt_account_rlp_bytes = Vec::new();
                    mpt_account.encode(&mut mpt_account_rlp_bytes);
                    //MPTに現在登録されているRLPを取得
                    let existing_mpt_val =
                        state.eth_trie.get(address_hash.as_slice()).unwrap_or(None);

                    // 更新すべきか判定
                    let should_insert = match existing_mpt_val {
                        None => true, // MPTに存在しない（新規アカウント）なら絶対に挿入
                        Some(old_rlp) => {
                            // MPTに存在するなら、RLPの中身が変化している場合のみ挿入
                            old_rlp != mpt_account_rlp_bytes
                        }
                    };

                    // 更新
                    if should_insert {
                        tracing::debug!("更新: 0x{}", hex::encode(address));
                        let _ = state.eth_trie.remove(address_hash.as_slice());
                        state
                            .eth_trie
                            .insert(address_hash.as_slice(), mpt_account_rlp_bytes.as_slice())
                            .unwrap();
                    }
                }
                //eth_trieのルートハッシュを取得
                let new_state_root = state.eth_trie.root_hash().unwrap();
                state.update_eth_trie(new_state_root);
                Err((final_billed_gas, Vec::new()))
            }
        }
    }
}
