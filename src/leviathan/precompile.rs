#![allow(dead_code)]
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::VersionId;
use crate::my_trait::evm_trait::Ofunction;
use crate::my_trait::leviathan_trait::CompiledContract;
use alloy_primitives::{U256, uint};
use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G2Affine};
use ark_ec::pairing::Pairing;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{BigInteger, One, PrimeField, Zero};
use num_bigint::BigUint;
use ripemd::{Digest as _, Ripemd160};
use secp256k1::Secp256k1;
use sha2::{Digest as _, Sha256};
use sha3::{Digest as _, Keccak256};
use std::ops::Mul;
use std::ops::Rem;

pub const SECP256K1N: U256 =
    uint!(0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141_U256);
pub const PRIME_P: U256 =
    uint!(21888242871839275222246405745257275088696311157297823662689037894645226208583_U256);

impl CompiledContract for LEVIATHAN {
    #[inline(never)]
    fn ecrec(gas: U256, data: &[u8]) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        // ガス検証
        if gas < U256::from(3000) {
            return Err((U256::ZERO, None));
        }
        let return_gas = gas - U256::from(3000);

        //データ抽出
        let mut tmp = [0u8; 128];
        let copy_len = data.len().min(128);
        tmp[..copy_len].copy_from_slice(&data[..copy_len]);
        let h: [u8; 32] = tmp[0..32].try_into().expect("[ecrec]変換失敗");
        let v: [u8; 32] = tmp[32..64].try_into().expect("[ecrec]変換失敗");
        let r: [u8; 32] = tmp[64..96].try_into().expect("[ecrec]変換失敗");
        let s: [u8; 32] = tmp[96..128].try_into().expect("[ecrec]変換失敗");

        //バリデーション
        // 1.vの条件 27 or 28
        let mut v_val = U256::from_be_bytes(v);
        if v_val != U256::from(27) && v_val != U256::from(28) {
            return Ok((return_gas, Vec::<u8>::new()));
        }
        // 2. rとsの条件
        let r_val = U256::from_be_bytes(r);
        let s_val = U256::from_be_bytes(s);
        let r_check = U256::ZERO < r_val && r_val < SECP256K1N;
        let s_check = U256::ZERO < s_val && s_val < SECP256K1N;
        if !r_check || !s_check {
            return Ok((return_gas, Vec::<u8>::new()));
        }
        // v,r,sから復元可能な署名
        // 1. vの変換
        v_val -= U256::from(27);
        let v_val_i32 = v_val.to::<u32>() as i32;
        let recovery_id = match secp256k1::ecdsa::RecoveryId::try_from(v_val_i32) {
            Ok(id) => id,
            Err(_) => return Ok((return_gas, Vec::<u8>::new())),
        };
        // 2. rとsを結合
        let mut r_s = [0u8; 64];
        r_s[0..32].copy_from_slice(&r[..]);
        r_s[32..64].copy_from_slice(&s[..]);
        // 3. messageの生成
        let message = match secp256k1::Message::from_digest_slice(&h) {
            Ok(message) => message,
            Err(_) => return Ok((return_gas, Vec::<u8>::new())),
        };
        // 4. 署名の生成
        let signature =
            match secp256k1::ecdsa::RecoverableSignature::from_compact(&r_s, recovery_id) {
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
        let result: [u8; 32] = hasher.finalize().into();
        let mut return_data = vec![0u8; 32];
        return_data[12..].copy_from_slice(&result[12..]);
        Ok((return_gas, return_data))
    }

    #[inline(never)]
    fn sha256(gas: U256, data: &[u8]) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        // 1. 必要ガスの計算: 60 + 12 * ceil(|data| / 32)
        // 整数演算での切り上げ: (len + 31) / 32
        let word_count = data.len().div_ceil(32);
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
        let word_count = data.len().div_ceil(32);
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
        let word_count = data.len().div_ceil(32);
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
        data: &[u8],
        version: VersionId,
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        if data.is_empty() {
            let used_gas = if version >= VersionId::Berlin {
                U256::from(200) // Berlin以降は最低200ガス
            } else {
                U256::ZERO // Byzantium等は0ガス
            };

            // OOGチェック
            if gas < used_gas {
                return Err((U256::ZERO, None)); // ガス欠
            }

            // ステータス「成功(Ok)」で、残ガスと空の配列を返す
            return Ok((gas - used_gas, Vec::new()));
        }
        //ヘルパー関数
        let get_padded_data = |start: usize, len: usize| -> Vec<u8> {
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
        let b_len = U256::from_be_slice(&b_len_byte);
        let e_len = U256::from_be_slice(&e_len_byte);
        let m_len = U256::from_be_slice(&m_len_byte);

        //ガス計算
        //f(x)
        let max_len = b_len.max(m_len);
        let words = (max_len + U256::from(7)) / U256::from(8);
        let val1 = words.saturating_mul(max_len);

        //val2
        let val2 = if e_len <= U256::from(32) {
            let e_len_usize = e_len.try_into().unwrap_or(0);
            let b_len_usize = e_len.try_into().unwrap_or(usize::MAX);
            let e_bytes = get_padded_data(96 + b_len_usize, e_len_usize);
            let e_val_u256 = U256::from_be_slice(&e_bytes);
            if e_val_u256.is_zero() {
                U256::from(1)
            } else {
                U256::from(e_val_u256.bit_len())
            }
        } else {
            let _e_len_usize = e_len.try_into().unwrap_or(0);
            let b_len_usize = e_len.try_into().unwrap_or(usize::MAX);
            let e_top_bytes = get_padded_data(96 + b_len_usize, 32);
            let e_top = U256::from_be_slice(&e_top_bytes);
            let rest = e_len - U256::from(32);

            (rest * U256::from(8)) + U256::from(e_top.bit_len())
        };
        let g_cost1 = if version >= VersionId::Berlin {
            (val1 * val2) / U256::from(3)
        } else {
            (val1 * val2) / U256::from(20)
        };
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
        } else {
            BigUint::from_bytes_be(&b_val_byte)
        };
        let e_val = if e_len_usize == 0 {
            BigUint::ZERO
        } else {
            BigUint::from_bytes_be(&e_val_byte)
        };
        let m_val = if m_len_usize == 0 {
            BigUint::ZERO
        } else {
            BigUint::from_bytes_be(&m_val_byte)
        };

        //計算
        //例外を処理
        if m_val == BigUint::ZERO {
            return Ok((return_gas, vec![0u8; m_len_usize]));
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
        Ok((return_gas, result_val_byte))
    }

    fn bn_add(
        gas: U256,
        data: &[u8],
        version: VersionId,
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        //ヘルパー関数
        let get_padded_data = |start: usize, len: usize| -> Vec<u8> {
            let mut out = vec![0u8; len];
            if start < data.len() {
                let copy_len = (data.len() - start).min(len);
                out[..copy_len].copy_from_slice(&data[start..start + copy_len]);
            }
            out
        };
        // ガス検証
        let return_gas = if version >= VersionId::Istanbul {
            if gas < U256::from(150) {
                return Err((U256::ZERO, None));
            }
            gas - U256::from(150)
        } else {
            if gas < U256::from(500) {
                return Err((U256::ZERO, None));
            }
            gas - U256::from(500)
        };

        //データ抽出
        let x1_byte = get_padded_data(0, 32);
        let y1_byte = get_padded_data(32, 32);
        let x2_byte = get_padded_data(64, 32);
        let y2_byte = get_padded_data(96, 32);
        let x1 = U256::from_be_slice(&x1_byte);
        let y1 = U256::from_be_slice(&y1_byte);
        let x2 = U256::from_be_slice(&x2_byte);
        let y2 = U256::from_be_slice(&y2_byte);
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
        } else {
            let point = G1Affine::new_unchecked(fq_x1, fq_y1);
            if !point.is_on_curve() {
                return Err((U256::ZERO, None));
            }
            point
        };
        //  2.1 (x2, y2)
        let p2 = if x2 == U256::ZERO && y2 == U256::ZERO {
            G1Affine::zero()
        } else {
            let point = G1Affine::new_unchecked(fq_x2, fq_y2);
            if !point.is_on_curve() {
                return Err((U256::ZERO, None));
            }
            point
        };
        //p3を作成 (p1 + p2)
        let p3_proj = p1.into_group() + p2.into_group(); //into_group():マフィン座標から射影座標に変換
        let p3_affine = p3_proj.into_affine(); //into_affine(): 射影座標からマフィン座標へ

        //計算結果を返す
        if p3_affine.is_zero() {
            return Ok((return_gas, vec![0u8; 64]));
        }
        let x3_bytes = p3_affine.x.into_bigint().to_bytes_be();
        let y3_bytes = p3_affine.y.into_bigint().to_bytes_be();
        let mut tmp = vec![0u8; 64];
        tmp[32 - x3_bytes.len()..32].copy_from_slice(&x3_bytes[..]);
        tmp[64 - y3_bytes.len()..64].copy_from_slice(&y3_bytes[..]);

        Ok((return_gas, tmp))
    }

    fn bn_mul(
        gas: U256,
        data: &[u8],
        version: VersionId,
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        //ヘルパー関数
        let get_padded_data = |start: usize, len: usize| -> Vec<u8> {
            let mut out = vec![0u8; len];
            if start < data.len() {
                let copy_len = (data.len() - start).min(len);
                out[..copy_len].copy_from_slice(&data[start..start + copy_len]);
            }
            out
        };
        // ガス検証
        let return_gas = if version >= VersionId::Istanbul {
            if gas < U256::from(6000) {
                return Err((U256::ZERO, None));
            }
            gas - U256::from(6000)
        } else {
            if gas < U256::from(40000) {
                return Err((U256::ZERO, None));
            }
            gas - U256::from(40000)
        };

        //データ抽出
        let x_byte = get_padded_data(0, 32);
        let y_byte = get_padded_data(32, 32);
        let n_byte = get_padded_data(64, 32);
        let x = U256::from_be_slice(&x_byte);
        let y = U256::from_be_slice(&y_byte);
        let _n = U256::from_be_slice(&n_byte);
        let fq_x = Fq::from_be_bytes_mod_order(&x_byte);
        let fq_y = Fq::from_be_bytes_mod_order(&y_byte);
        let scalar_n = Fr::from_be_bytes_mod_order(&n_byte);

        //バリデーション要件
        // 1. フィールドサイズの検証
        if x >= PRIME_P || y >= PRIME_P {
            return Err((U256::ZERO, None));
        }

        // 2. 曲線状にあるかの検証
        //  2.1 (x1, y1)
        let p = if x == U256::ZERO && y == U256::ZERO {
            G1Affine::zero()
        } else {
            let point = G1Affine::new_unchecked(fq_x, fq_y);
            if !point.is_on_curve() {
                return Err((U256::ZERO, None));
            }
            point
        };
        //計算
        let p_result_proj = p.into_group().mul(scalar_n);
        let p_result_affine = p_result_proj.into_affine();
        //計算結果を返す
        if p_result_affine.is_zero() {
            return Ok((return_gas, vec![0u8; 64]));
        }
        let result_bytes_x = p_result_affine.x.into_bigint().to_bytes_be();
        let result_bytes_y = p_result_affine.y.into_bigint().to_bytes_be();
        let mut tmp = vec![0u8; 64];
        tmp[32 - result_bytes_x.len()..32].copy_from_slice(&result_bytes_x[..]);
        tmp[64 - result_bytes_y.len()..64].copy_from_slice(&result_bytes_y[..]);

        Ok((return_gas, tmp))
    }

    fn bn_pairing(
        gas: U256,
        data: &[u8],
        version: VersionId,
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        //要件確認1
        if data.len().rem(192) != 0 {
            return Err((U256::ZERO, None));
        }
        let k = data.len() / 192;

        // ガス検証
        let used_gas = if version >= VersionId::Istanbul {
            U256::from(34000)
                .saturating_mul(U256::from(k))
                .saturating_add(U256::from(45000))
        } else {
            U256::from(80000)
                .saturating_mul(U256::from(k))
                .saturating_add(U256::from(100000))
        };

        if gas < used_gas {
            return Err((U256::ZERO, None));
        }
        let return_gas = gas - used_gas;
        //データ取得
        let mut g1_points = Vec::new();
        let mut g2_points = Vec::new();
        let mut i: usize = 0;
        while i < k {
            let offset = i * 192;
            //G1の抽出
            let g1_x = Fq::from_be_bytes_mod_order(&data[offset..offset + 32]);
            let g1_y = Fq::from_be_bytes_mod_order(&data[offset + 32..offset + 64]);
            let x = U256::from_be_slice(&data[offset..offset + 32]);
            let y = U256::from_be_slice(&data[offset + 32..offset + 64]);
            //バリデーション(G1)
            // 1. フィールドサイズの検証
            if x >= PRIME_P || y >= PRIME_P {
                return Err((U256::ZERO, None));
            }

            // 2. 曲線状にあるかの検証
            let p = if x == U256::ZERO && y == U256::ZERO {
                G1Affine::zero()
            } else {
                let point = G1Affine::new_unchecked(g1_x, g1_y);
                if !point.is_on_curve() {
                    return Err((U256::ZERO, None));
                }
                point
            };
            g1_points.push(p);

            //G2の抽出
            let x_im = Fq::from_be_bytes_mod_order(&data[offset + 64..offset + 96]);
            let x_re = Fq::from_be_bytes_mod_order(&data[offset + 96..offset + 128]);
            let y_im = Fq::from_be_bytes_mod_order(&data[offset + 128..offset + 160]);
            let y_re = Fq::from_be_bytes_mod_order(&data[offset + 160..offset + 192]);
            let x_im_u256 = U256::from_be_slice(&data[offset + 64..offset + 96]);
            let x_re_u256 = U256::from_be_slice(&data[offset + 96..offset + 128]);
            let y_im_u256 = U256::from_be_slice(&data[offset + 128..offset + 160]);
            let y_re_u256 = U256::from_be_slice(&data[offset + 160..offset + 192]);

            // Arkworksの Fq2::new は (実部, 虚部) の順に受け取る
            let fq2_x = Fq2::new(x_re, x_im);
            let fq2_y = Fq2::new(y_re, y_im);

            //バリデーション(G1)
            // 1. フィールドサイズの検証
            if x_im_u256 >= PRIME_P
                || x_re_u256 >= PRIME_P
                || y_im_u256 >= PRIME_P
                || y_re_u256 >= PRIME_P
            {
                return Err((U256::ZERO, None));
            }

            if x_im_u256 >= PRIME_P
                || x_re_u256 >= PRIME_P
                || y_im_u256 >= PRIME_P
                || y_re_u256 >= PRIME_P
            {
                return Err((U256::ZERO, None));
            } // 2. 曲線状にあるかの検証
            let p2 = if fq2_x.is_zero() && fq2_y.is_zero() {
                G2Affine::zero() // G2の無限遠点
            } else {
                let point = G2Affine::new_unchecked(fq2_x, fq2_y);

                // ZKの安全性のため、G2では曲線チェックに加えてサブグループチェックも行うのが一般的です
                if !point.is_on_curve() || !point.is_in_correct_subgroup_assuming_on_curve() {
                    return Err((U256::ZERO, None));
                }
                point
            };
            g2_points.push(p2);

            i += 1;
        }

        //ペアリング計算
        let pairing_result = Bn254::multi_pairing(g1_points, g2_points);
        //等式の検証
        let is_valid = pairing_result.0.is_one();
        let mut output = vec![0u8; 32];
        if is_valid {
            output[31] = 1;
        }

        Ok((return_gas, output))
    }
}
