use crate::leviathan::structs::{Transaction, BlsTransaction};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};


#[derive(Debug, Clone)]
pub enum TransactionEnvelope {
    Legacy(Transaction),
    Bls(BlsTransaction),
}

impl alloy_rlp::Decodable for TransactionEnvelope {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let first_byte = buf.first().copied().ok_or(alloy_rlp::Error::InputTooShort)?;

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
    fn get_nonce(&self) -> usize {
        
