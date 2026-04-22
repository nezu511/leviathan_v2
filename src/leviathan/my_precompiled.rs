#![allow(dead_code)]
use crate::leviathan::leviathan::LEVIATHAN;
use crate::leviathan::structs::VersionId;
use crate::my_trait::leviathan_trait::MCC;
use alloy_primitives::{U256, uint};
use rsa::{Pkcs1v15Sign, RsaPublicKey};
use num_bigint::BigUint;
use sha2::{Digest as _, Sha256};


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

        //データは十分か
        if data.len() < 544 {
            tracing::warn!("[my_rsa] 入力データが不適切");
            return Err((U256::ZERO, None));
        }

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
        let exponent_byte = get_padded_data(544, data.len() - 544);

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
}
