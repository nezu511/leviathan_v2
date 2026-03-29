#![allow(dead_code)]

use alloy_primitives::{I256, U256};
use crate::my_trait::leviathan_trait::{State, TransactionExecution, RoleBack, CompiledContract};
use crate::my_trait::evm_trait::{Xi, Gfunction, Zfunction, Hfunction, Ofunction};
use crate::leviathan::world_state::{WorldState, Address, Account};
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{SubState, ExecutionEnvironment, Log, Transaction, BlockHeader, BackupSubstate};
use crate::evm::evm::EVM;
use sha3::Keccak256;
use sha2::Sha256;
use ripemd::{Ripemd160, Digest};
use std::collections::HashMap;

impl CompiledContract for LEVIATHAN {
    fn sha256(gas: U256, data: &[u8]) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        // 1. 必要ガスの計算: 60 + 12 * ceil(|data| / 32)
        // 整数演算での切り上げ: (len + 31) / 32
        let word_count = (data.len() + 31) / 32;
        let gas_required = U256::from(60) + U256::from(12 * word_count);

        // 2. Out-of-Gas (OOG) 検証
        if gas < gas_required {
            return Err((U256::ZERO, None));
        }

        // 3. 残ガスの計算
        let remaining_gas = gas - gas_required;

        // 4. SHA256ハッシュの計算
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize().to_vec();

        // 5. 残ガスと出力データを返
        Ok((remaining_gas, result))
    }

    fn precompile_ripemd160(gas: U256, data: &[u8]) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        // 1. 必要ガスの計算: 600 + 120 * ceil(|data| / 32) [cite: 1388]
        let word_count = (data.len() + 31) / 32;
        let gas_required = U256::from(600) + U256::from(120 * word_count);

        // 2. Out-of-Gas (OOG) 検証
        if gas < gas_required {
            return Err((U256::ZERO, None));
        }

        // 3. 残ガスの計算
        let remaining_gas = gas - gas_required;

        // 4. RIPEMD-160ハッシュの計算
        let mut hasher = Ripemd160::new();
        hasher.update(data);
        let hash_result = hasher.finalize();

        // 5. 出力フォーマットの調整 (32バイトにパディング)
        // o[0..11] = 0, o[12..31] = RIPEMD160(I_d)
        let mut result = vec![0u8; 32];
        result[12..32].copy_from_slice(&hash_result);

        // 6. 残ガスと出力データを返す
        Ok((remaining_gas, result))
    }


        // プリコンパイルコントラクト: Identity (Address 4)
        // Yellow Paper Appendix E (式230-233)
    fn precompile_identity(gas: U256, data: &[u8]) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        // 1. 必要ガスの計算: 15 + 3 * ceil(|data| / 32) [cite: 1397]
        let word_count = (data.len() + 31) / 32;
        let gas_required = U256::from(15) + U256::from(3 * word_count);

        // 2. Out-of-Gas (OOG) 検証
        if gas < gas_required {
            return Err((U256::ZERO, None));
        }

        // 3. 残ガスの計算
        let remaining_gas = gas - gas_required;

        // 4. 入力データをそのまま返す [cite: 1398]
        let result = data.to_vec();

        // 5. 残ガスと出力データを返す
        Ok((remaining_gas, result))
    }
}
