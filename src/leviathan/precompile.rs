#![allow(dead_code)]

use crate::evm::evm::EVM;
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::roleback::Action;
use crate::leviathan::structs::{
    BackupSubstate, BlockHeader, ExecutionEnvironment, Log, SubState, Transaction,
};
use crate::leviathan::world_state::{Account, Address, WorldState};
use crate::my_trait::evm_trait::{Gfunction, Hfunction, Ofunction, Xi, Zfunction};
use crate::my_trait::leviathan_trait::{CompiledContract, RoleBack, State, TransactionExecution};
use alloy_primitives::{I256, U256, uint};
use sha2::{Sha256, Digest as _};
use sha3::{Keccak256, Digest as _};
use ripemd::{Ripemd160, Digest as _};
use secp256k1::{
    Message, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use std::collections::HashMap;

pub const SECP256K1N: U256 = uint!(0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141_U256);

impl CompiledContract for LEVIATHAN {
    #[inline(never)]
    fn ecrec(gas: U256, data: &[u8]) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        // ガス検証
        if gas < U256::from(3000) {
            return Err((U256::ZERO, None));
        }
        let return_gas = gas - U256::from(3000);

        //データ抽出
        let mut tmp = [0u8;128];
        let copy_len = data.len().min(128);
        tmp[..copy_len].copy_from_slice(&data[..copy_len]);
        let h: [u8;32] = tmp[0..32].try_into().expect("[ecrec]変換失敗");
        let v: [u8;32] = tmp[32..64].try_into().expect("[ecrec]変換失敗");
        let r: [u8;32] = tmp[64..96].try_into().expect("[ecrec]変換失敗");
        let s: [u8;32] = tmp[96..128].try_into().expect("[ecrec]変換失敗");

        //バリデーション
        // 1.vの条件 27 or 28
        let mut v_val = U256::from_be_bytes(v);
        if v_val != U256::from(27) && v_val != U256::from(28) {
            return Ok((return_gas, Vec::<u8>::new()));
        }
        // 2. rとsの条件
        let r_val = U256::from_be_bytes(r);
        let s_val = U256::from_be_bytes(s);
        let r_check = U256::ZERO < r_val  && r_val < SECP256K1N;
        let s_check = U256::ZERO < s_val  && s_val < SECP256K1N;
        if !r_check || !s_check {
            return Ok((return_gas, Vec::<u8>::new()));
        }
        // v,r,sから復元可能な署名
        // 1. vの変換
        v_val = v_val - U256::from(27);
        let v_val_i32 = v_val.to::<u32>() as i32;
        let recovery_id = match secp256k1::ecdsa::RecoveryId::try_from(v_val_i32) {
            Ok(id) => id,
            Err(_) => return Ok((return_gas, Vec::<u8>::new())),
        };
        // 2. rとsを結合
        let mut r_s = [0u8;64];
        r_s[0..32].copy_from_slice(&r[..]);
        r_s[32..64].copy_from_slice(&s[..]);
        // 3. messageの生成
        let message = match secp256k1::Message::from_digest_slice(&h) {
            Ok(message) => message,
            Err(_) => return Ok((return_gas, Vec::<u8>::new())),
        };
        // 4. 署名の生成
        let signature =  match secp256k1::ecdsa::RecoverableSignature::from_compact(&r_s, recovery_id) {
            Ok(signature) => signature,
            Err(_) => return Ok((return_gas, Vec::<u8>::new())),
        };

        //公開鍵を復元
        let secp = Secp256k1::new();
        let publickey = match secp.recover_ecdsa(message, &signature) {
            Ok(pubkey) => pubkey,
            Err(_) => return Ok((return_gas, Vec::<u8>::new())),
        };
        let serialized_pk = publickey.serialize_uncompressed();
        let slice = &serialized_pk[1..65];
        //keccak256準備
        let mut hasher = Keccak256::new();
        hasher.update(slice);
        let result:[u8;32] = hasher.finalize().try_into().unwrap();
        let mut return_data = vec![0u8; 32];
        return_data[12..].copy_from_slice(&result[12..]);
        return Ok((gas, return_data));

    }

    #[inline(never)]
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
    #[inline(never)]
    fn precompile_ripemd160(
        gas: U256,
        data: &[u8],
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
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
    #[inline(never)]
    fn precompile_identity(
        gas: U256,
        data: &[u8],
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
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
