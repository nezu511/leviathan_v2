use crate::leviathan::structs::{BlsTransaction, Transaction};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use alloy_primitives::{B256, Bytes, TxKind, U256};

#[derive(Debug, Clone)]
pub enum TransactionEnvelope {
    Legacy(Transaction),
    Bls(BlsTransaction),
}

impl alloy_rlp::Decodable for TransactionEnvelope {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let first_byte = buf
            .first()
            .copied()
            .ok_or(alloy_rlp::Error::InputTooShort)?;

        if first_byte >= 0xc0 {
            // 先頭がリスト開始バイトなら、従来のEthereum互換トランザクション
            let legacy_tx = Transaction::decode(buf)?;
            Ok(TransactionEnvelope::Legacy(legacy_tx))
        } else {
            // 先頭がタイプIDなら、オリジナルフォーマット（例: タイプ 0x05 を BLS用とする）
            let tx_type = buf[0];
            *buf = &buf[1..]; // タイプIDの1バイトをスキップ

            match tx_type {
                0x05 => {
                    let bls_tx = BlsTransaction::decode(buf)?;
                    Ok(TransactionEnvelope::Bls(bls_tx))
                }
                _ => Err(alloy_rlp::Error::Custom("Unknown transaction type")),
            }
        }
    }
}

impl TransactionEnvelope {
    pub fn get_nonce(&self) -> usize {
        match self {
            TransactionEnvelope::Legacy(transaction) => return transaction.t_nonce,
            TransactionEnvelope::Bls(transaction) => return transaction.t_nonce,
        }
    }

    pub fn get_price(&self) -> U256 {
        match self {
            TransactionEnvelope::Legacy(transaction) => return transaction.t_price,
            TransactionEnvelope::Bls(transaction) => return transaction.t_price,
        }
    }

    pub fn get_gas_limit(&self) -> U256 {
        match self {
            TransactionEnvelope::Legacy(transaction) => return transaction.t_gas_limit,
            TransactionEnvelope::Bls(transaction) => return transaction.t_gas_limit,
        }
    }

    pub fn get_t_to(&self) -> TxKind {
        match self {
            TransactionEnvelope::Legacy(transaction) => return transaction.t_to,
            TransactionEnvelope::Bls(transaction) => return transaction.t_to,
        }
    }

    pub fn get_value(&self) -> U256 {
        match self {
            TransactionEnvelope::Legacy(transaction) => return transaction.t_value,
            TransactionEnvelope::Bls(transaction) => return transaction.t_value,
        }
    }

    pub fn get_data(&self) -> Bytes {
        match self {
            TransactionEnvelope::Legacy(transaction) => return transaction.data.clone(),
            TransactionEnvelope::Bls(transaction) => return transaction.data.clone(),
        }
    }

    pub fn get_t_w(&self) -> Option<U256> {
        match self {
            TransactionEnvelope::Legacy(transaction) => return Some(transaction.t_w),
            TransactionEnvelope::Bls(transaction) => return None,
        }
    }

    pub fn get_t_r(&self) -> Option<U256> {
        match self {
            TransactionEnvelope::Legacy(transaction) => return Some(transaction.t_r),
            TransactionEnvelope::Bls(transaction) => return None,
        }
    }

    pub fn get_t_s(&self) -> Option<U256> {
        match self {
            TransactionEnvelope::Legacy(transaction) => return Some(transaction.t_s),
            TransactionEnvelope::Bls(transaction) => return None,
        }
    }

    pub fn get_bls_signature(&self) -> Option<Bytes> {
        match self {
            TransactionEnvelope::Legacy(transaction) => return None,
            TransactionEnvelope::Bls(transaction) => return Some(transaction.bls_signature.clone()),
        }
    }


}
