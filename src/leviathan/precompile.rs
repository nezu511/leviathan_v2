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
use num_bigint::BigUint;
use std::collections::HashMap;
use ark_bn254::{Fq, G1Affine};
use ark_ff::{PrimeField, BigInteger};
use ark_ec::{AffineRepr, CurveGroup};

pub const SECP256K1N: U256 = uint!(0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141_U256);
pub const PRIME_P: U256 = uint!(21888242871839275222246405745257275088696311157297823662689037894645226208583_U256);

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

    // プリコンパイルコントラクト: Identity (Address 5)
    #[inline(never)]
    fn expmod(
        gas: U256,
        data: &[u8]
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        //ヘルパー関数
        let get_padded_data = |start: usize, len:usize| -> Vec<u8> {
            let mut out = vec![0u8; len];
            if start < data.len() {
                let copy_len = (data.len() - start).min(len);
                    out[..copy_len].copy_from_slice(&data[start..start + copy_len]);
            }
            out
        };
        //データ抽出
        let b_len_byte = get_padded_data(0, 32);
        let e_len_byte = get_padded_data(32, 32);
        let m_len_byte = get_padded_data(64, 32);
        let mut b_len = U256::from_be_slice(&b_len_byte);
        let mut e_len = U256::from_be_slice(&e_len_byte);
        let mut m_len = U256::from_be_slice(&m_len_byte);

        //ガス計算
        //f(x)
        let max_len = b_len.max(m_len);
        let words = (max_len + U256::from(7))/U256::from(8);
        let val1 = words.saturating_mul(max_len);
    
        //val2
        let val2 = if e_len <= U256::from(32) {
            let e_len_usize = e_len.try_into().unwrap_or(0);
            let b_len_usize = e_len.try_into().unwrap_or(usize::MAX);
            let e_bytes = get_padded_data(96 + b_len_usize, e_len_usize);
            let e_val_u256 = U256::from_be_slice(&e_bytes);
            if e_val_u256.is_zero() {
                U256::from(1)
            }else{
                U256::from(e_val_u256.bit_len())
            }
        }else{
            let e_len_usize = e_len.try_into().unwrap_or(0);
            let b_len_usize = e_len.try_into().unwrap_or(usize::MAX);
            let e_top_bytes = get_padded_data(96 + b_len_usize, 32);
            let e_top = U256::from_be_slice(&e_top_bytes);
            let rest = e_len - U256::from(32);

            (rest * U256::from(8)) + U256::from(e_top.bit_len())
        };
        let g_cost1 = (val1 * val2) / U256::from(3);
        let used_gas = g_cost1.max(U256::from(200));
        // ガス検証
        if gas < used_gas {
            return Err((U256::ZERO, None));
        }
        let return_gas = gas - used_gas;
        
        //データを取得
        //1. データ長を取得
        let b_len_usize = b_len.try_into().unwrap_or(0);
        let e_len_usize = e_len.try_into().unwrap_or(0);
        let m_len_usize = m_len.try_into().unwrap_or(0);
        //2. データをVecで取得
        let b_val_byte = get_padded_data(96, b_len_usize);
        let e_val_byte = get_padded_data(96 + b_len_usize, e_len_usize);
        let m_val_byte = get_padded_data(96 + b_len_usize + e_len_usize, m_len_usize);
        //3. VecデータをBigUintに変換
        let b_val = if b_len_usize == 0 {
            BigUint::ZERO
        }else{
            BigUint::from_bytes_be(&b_val_byte)
        };
        let e_val = if e_len_usize == 0 {
            BigUint::ZERO
        }else{
            BigUint::from_bytes_be(&e_val_byte)
        };
        let m_val = if m_len_usize == 0 {
            BigUint::ZERO
        }else{
            BigUint::from_bytes_be(&m_val_byte)
        };

        //計算
        //例外を処理
        if m_val == BigUint::ZERO {
            return Ok((return_gas, vec![0u8;m_len_usize]));
        }
        let result_val = b_val.modpow(&e_val, &m_val);
        let mut result_val_byte = result_val.to_bytes_be();

        if result_val_byte.len() < m_len_usize {
            let padding = m_len_usize - result_val_byte.len();
            let mut padded_result = vec![0u8; padding];
            padded_result.append(&mut result_val_byte);
            return Ok((return_gas, padded_result));
        } else if result_val_byte.len() > m_len_usize {
            let start = result_val_byte.len() - m_len_usize;
            return Ok((return_gas, result_val_byte[start..].to_vec()));
        }
        return Ok((return_gas, result_val_byte));

    }

    fn bn_add(
        gas: U256,
        data: &[u8]
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        //ヘルパー関数
        let get_padded_data = |start: usize, len:usize| -> Vec<u8> {
            let mut out = vec![0u8; len];
            if start < data.len() {
                let copy_len = (data.len() - start).min(len);
                    out[..copy_len].copy_from_slice(&data[start..start + copy_len]);
            }
            out
        };
        // ガス検証
        if gas < U256::from(150) {
            return Err((U256::ZERO, None));
        }
        let return_gas = gas - U256::from(150);
        //データ抽出
        let x1_byte = get_padded_data(0, 32);
        let y1_byte = get_padded_data(32, 32);
        let x2_byte = get_padded_data(64, 32);
        let y2_byte = get_padded_data(96, 32);
        let mut x1 = U256::from_be_slice(&x1_byte);
        let mut y1 = U256::from_be_slice(&y1_byte);
        let mut x2 = U256::from_be_slice(&x2_byte);
        let mut y2 = U256::from_be_slice(&y2_byte);
        let fq_x1 = Fq::from_be_bytes_mod_order(&x1_byte);
        let fq_y1 = Fq::from_be_bytes_mod_order(&y1_byte);
        let fq_x2 = Fq::from_be_bytes_mod_order(&x2_byte);
        let fq_y2 = Fq::from_be_bytes_mod_order(&y2_byte);

        //バリデーション要件
        // 1. フィールドサイズの検証
        if x1 >= PRIME_P || y1 >= PRIME_P || x2 >= PRIME_P || y2 >= PRIME_P {
            return Err((U256::ZERO, None));
        }

        // 2. 曲線状にあるかの検証
        //  2.1 (x1, y1)
        let p1 = if x1 == U256::ZERO && y1 == U256::ZERO {
            G1Affine::zero()
        }else{
            let point = G1Affine::new_unchecked(fq_x1, fq_y1);
            if !point.is_on_curve() {
                return Ok((return_gas, Vec::<u8>::new()));
            }
            point
        };
        //  2.1 (x2, y2)
        let p2 = if x2 == U256::ZERO && y2 == U256::ZERO {
            G1Affine::zero()
        }else{
            let point = G1Affine::new_unchecked(fq_x2, fq_y2);
            if !point.is_on_curve() {
                return Ok((return_gas, Vec::<u8>::new()));
            }
            point
        };
        //p3を作成 (p1 + p2)
        let p3_proj = p1.into_group() + p2.into_group();    //into_group():マフィン座標から射影座標に変換
        let p3_affine = p3_proj.into_affine();              //into_affine(): 射影座標からマフィン座標へ

        //計算結果を返す
        if p3_affine.is_zero() {
            return Ok((return_gas, vec![0u8,64]));
        }
        let x3_bytes = p3_affine.x.into_bigint().to_bytes_be();
        let y3_bytes = p3_affine.y.into_bigint().to_bytes_be();
        let mut tmp = vec![0u8,64];
        tmp[..32].copy_from_slice(&x3_bytes[..]);
        tmp[32..].copy_from_slice(&y3_bytes[..]);
            
        return Ok((return_gas, tmp));

    }


}
