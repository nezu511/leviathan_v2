#![allow(dead_code)]
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::precompile::PRIME_P;
use crate::leviathan::structs::VersionId;
use crate::my_trait::leviathan_trait::MCC;
use alloy_primitives::{U256, uint};
use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G2Affine};
use ark_ec::pairing::Pairing;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{BigInteger, One, PrimeField, Zero};
use ark_groth16::VerifyingKey;
use ark_serialize::CanonicalDeserialize;
use ark_snark::SNARK;
use light_poseidon::{Poseidon, PoseidonHasher};
use num_bigint::BigUint;
use rsa::{Pkcs1v15Sign, RsaPublicKey};
use sha2::{Digest as _, Sha256};
use std::ops::Rem;

const WORD_SIZE: usize = 32;

impl MCC for LEVIATHAN {
    fn my_rsa(
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

        //使用ガス量を計算
        let gas_required = U256::from(168000);
        // Out-of-Gas (OOG) 検証
        if gas < gas_required {
            tracing::warn!("[my_rsa] OOG");
            return Err((U256::ZERO, None));
        }
        let return_gas = gas - gas_required;
        //データ抽出
        let signature_byte = get_padded_data(0, 256);
        let modulus_byte = get_padded_data(256, 256);
        let message_byte = get_padded_data(512, 32);
        let exp_len = data.len().saturating_sub(544);
        let exponent_byte = get_padded_data(544, exp_len);

        //BigUintへの変換
        let n = rsa::BigUint::from_bytes_be(&modulus_byte);
        let e = rsa::BigUint::from_bytes_be(&exponent_byte);

        let Ok(public_key) = RsaPublicKey::new(n, e) else {
            tracing::warn!("[my_rsa] RsaPublicKey生成失敗");
            return Err((U256::ZERO, None));
        };

        // 4. PKCS#1 v1.5 による署名検証
        let scheme = Pkcs1v15Sign::new::<Sha256>();
        let is_valid = public_key
            .verify(scheme, &message_byte, &signature_byte)
            .is_ok();
        let mut output = vec![0u8; 32];
        if is_valid {
            output[31] = 1;
        }

        Ok((return_gas, output))
    }

    fn my_groth16(
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
        //検証キーを取得する
        if data.is_empty() {
            //要件確認1
            tracing::warn!("[my_groth16] 検証キーの取得でエラー（データ長が0)");
            return Err((U256::ZERO, None));
        }
        //key長を取得する
        let mut key_len_bytes = get_padded_data(0, WORD_SIZE);
        let key_len_u256 = U256::from_be_slice(&key_len_bytes);
        let Ok(key_len) = usize::try_from(key_len_u256) else {
            //要件確認2
            tracing::warn!("[my_groth16] 検証キーの取得でエラー（U256→ usizeで失敗)");
            return Err((U256::ZERO, None));
        };
        if key_len > data.len() {
            //要件確認3
            tracing::warn!("[my_groth16] 検証キーの取得でエラー（kye_lenがdata長を超えている)");
            return Err((U256::ZERO, None));
        };
        //key_bytesを取得する
        let mut key_bytes = get_padded_data(32, key_len);

        //境界を定義
        let proof_offset = 32 + key_len;
        let pub_input_offset = proof_offset + 256;

        // 公開入力を抽出
        let mut input_data = get_padded_data(
            pub_input_offset,
            data.len().saturating_sub(pub_input_offset),
        );
        //proofの検証を行う
        if input_data.len().rem(WORD_SIZE) != 0 {
            return Err((U256::ZERO, None));
        }
        let k = input_data.len() / WORD_SIZE;

        //ガスチェック!!(とりあえず）
        let used_gas = U256::from(34000)
            .saturating_mul(U256::from(k))
            .saturating_add(U256::from(45000));

        if gas < used_gas {
            return Err((U256::ZERO, None)); // Out of Gas
        }
        let return_gas = gas - used_gas;

        //Proofを取得する．
        let proof_size = 256;
        let mut zk_data = get_padded_data(proof_offset, proof_size);
        //proofの検証を行う
        if zk_data.len().rem(WORD_SIZE) != 0 || zk_data.len() != proof_size {
            return Err((U256::ZERO, None));
        }
        //G1 pointを作成
        let get_g1_point = |offset: usize| -> Result<G1Affine, ()> {
            let g1_x = Fq::from_be_bytes_mod_order(&zk_data[offset..offset + 32]);
            let g1_y = Fq::from_be_bytes_mod_order(&zk_data[offset + 32..offset + 64]);
            let x = U256::from_be_slice(&zk_data[offset..offset + 32]);
            let y = U256::from_be_slice(&zk_data[offset + 32..offset + 64]);
            //バリデーション(G1)
            // 1. フィールドサイズの検証
            if x >= PRIME_P || y >= PRIME_P {
                return Err(());
            }

            // 2. 曲線状にあるかの検証
            let proof_g1 = if x == U256::ZERO && y == U256::ZERO {
                G1Affine::zero()
            } else {
                let point = G1Affine::new_unchecked(g1_x, g1_y);
                if !point.is_on_curve() {
                    return Err(());
                }
                point
            };
            return Ok(proof_g1);
        };

        //G2の抽出
        let get_g2_point = || -> Result<G2Affine, ()> {
            let x_im = Fq::from_be_bytes_mod_order(&zk_data[64..96]);
            let x_re = Fq::from_be_bytes_mod_order(&zk_data[96..128]);
            let y_im = Fq::from_be_bytes_mod_order(&zk_data[128..160]);
            let y_re = Fq::from_be_bytes_mod_order(&zk_data[160..192]);
            let x_im_u256 = U256::from_be_slice(&zk_data[64..96]);
            let x_re_u256 = U256::from_be_slice(&zk_data[96..128]);
            let y_im_u256 = U256::from_be_slice(&zk_data[128..160]);
            let y_re_u256 = U256::from_be_slice(&zk_data[160..192]);

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
                return Err(());
            }

            // 2. 曲線状にあるかの検証
            let proof_g2 = if fq2_x.is_zero() && fq2_y.is_zero() {
                G2Affine::zero() // G2の無限遠点
            } else {
                let point = G2Affine::new_unchecked(fq2_x, fq2_y);

                // ZKの安全性のため、G2では曲線チェックに加えてサブグループチェックも行うのが一般的です
                if !point.is_on_curve() || !point.is_in_correct_subgroup_assuming_on_curve() {
                    return Err(());
                }
                point
            };
            return Ok(proof_g2);
        };

        let Ok(point_a) = get_g1_point(0) else {
            return Err((U256::ZERO, None));
        };
        let Ok(point_b) = get_g2_point() else {
            return Err((U256::ZERO, None));
        };
        let Ok(point_c) = get_g1_point(192) else {
            return Err((U256::ZERO, None));
        };

        //proofを作成
        let proof = ark_groth16::Proof {
            a: point_a,
            b: point_b,
            c: point_c,
        };

        //VerifiyingKey構造体を作成
        let Ok(vk) = VerifyingKey::<Bn254>::deserialize_uncompressed(&*key_bytes) else {
            tracing::warn!("[my_groth16] VKのデシリアライズに失敗");
            return Err((U256::ZERO, None));
        };
        let pvk = ark_groth16::prepare_verifying_key(&vk);

        // 公開入力を抽出
        let mut public_inputs = Vec::new();
        let mut i: usize = 0;
        while i < k {
            let offset = i * WORD_SIZE;
            let input_bytes = get_padded_data(offset + pub_input_offset, WORD_SIZE);
            let fr = Fr::from_be_bytes_mod_order(&input_bytes);
            public_inputs.push(fr);
            i += 1;
        }

        let is_valid = ark_groth16::Groth16::<Bn254>::verify_proof(&pvk, &proof, &public_inputs)
            .unwrap_or(false);
        let mut output = vec![0u8; WORD_SIZE];
        if is_valid {
            output[31] = 1;
        }

        Ok((return_gas, output))
    }

    fn my_poseidon(
        gas: U256,
        data: &[u8],
        version: VersionId,
    ) -> Result<(U256, Vec<u8>), (U256, Option<Vec<u8>>)> {
        let input_datas_len = data.len() - 1;
        //検証キーを取得する
        if data.is_empty() || input_datas_len.rem(WORD_SIZE) != 0 {
            //要件確認1
            tracing::warn!("[my_poseidon] 入力データが不適切");
            return Err((U256::ZERO, None));
        }
        let k = input_datas_len / WORD_SIZE;
        //light-poseidonがサポートする範囲かどうか
        if k < 1 || k > 12 {
            tracing::warn!("[my_poseidon] 要素数が不適切");
            return Err((U256::ZERO, None));
        }

        //ガス（暫定)
        let used_gas = U256::from(30000).saturating_add(U256::from(5000) * U256::from(k));
        if gas < used_gas {
            return Err((U256::ZERO, None));
        }
        let return_gas = gas - used_gas;

        let mut input_datas = vec![0u8; input_datas_len];
        input_datas.copy_from_slice(&data[1..]);
        // 公開入力を抽出
        let mut elements = Vec::new();
        let mut i: usize = 0;
        while i < k {
            let offset = i * WORD_SIZE;
            let mut tmp = vec![0u8; 32];
            tmp.copy_from_slice(&input_datas[offset..offset + WORD_SIZE]);
            let tmp_u256 = U256::from_be_slice(&tmp);
            if tmp_u256 >= PRIME_P {
                tracing::warn!("[my_poseidon] 要素が位数P以上の値");
                return Err((U256::ZERO, None));
            }
            let fr = Fr::from_be_bytes_mod_order(&tmp);
            elements.push(fr);
            i += 1;
        }

        let catalog_id = data[0];

        let mut poseidon = match catalog_id {
            0x01 => Poseidon::<Fr>::new_circom(k).unwrap(),
            _ => {
                tracing::warn!("[my_poseidon] 未知のカタログID");
                return Err((U256::ZERO, None));
            }
        };

        // 5. ハッシュ計算を実行！
        let Ok(hash_result) = poseidon.hash(&elements) else {
            tracing::warn!("[my_poseidon] hash計算エラー");
            return Err((U256::ZERO, None));
        };

        // 6. 出力フォーマットの調整 (Fr を 32バイトのBig Endianに変換)
        let mut output = vec![0u8; WORD_SIZE];
        let result_bytes = hash_result.into_bigint().to_bytes_be();
        // ゼロパディングして右詰めで配置
        output[WORD_SIZE - result_bytes.len()..].copy_from_slice(&result_bytes);

        Ok((return_gas, output))
    }
}
