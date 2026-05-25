use crate::LeviathanApp;
use alloy_primitives::{Address, B256, Bloom, BloomInput, Log as PrimitiveLog, TxKind, U256};
use alloy_rlp::{Encodable, Header};
use alloy_rpc_types::Filter;
use bytes::BytesMut;
use leviathan_v2::leviathan::structs::{Transaction, VersionId};
use leviathan_v2::my_trait::leviathan_trait::State;
use secp256k1::{
    Message, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use sha3::{Digest, Keccak256};

pub fn get_sender(transaction: &Transaction) -> Option<Address> {
    let Ok(t_w_u64) = u64::try_from(transaction.t_w) else {
        tracing::warn!("t_w is too large for u64");
        return None;
    };
    let (recovery_id_u8, chain_id) = if t_w_u64 == 27 || t_w_u64 == 28 {
        ((t_w_u64 - 27) as u8, None)
    } else if t_w_u64 >= 35 {
        (((t_w_u64 - 35) % 2) as u8, Some((t_w_u64 - 35) / 2))
    } else {
        tracing::warn!("[get_sender] Invalid v value");
        return None;
    };
    let Ok(recovery_id) = RecoveryId::try_from(recovery_id_u8 as i32) else {
        tracing::warn!("Invalid recovery id");
        return None;
    };
    // 1. 各要素のRLPペイロード長を事前計算する (alloy-rlpの特徴)
    let mut payload_length = 0;
    payload_length += transaction.t_nonce.length();
    payload_length += transaction.t_price.length();
    payload_length += transaction.t_gas_limit.length();

    let to_slice = match &transaction.t_to {
        TxKind::Call(address) => address.0.as_slice(),
        TxKind::Create => &[], // 空のバイト列
    };
    payload_length += to_slice.length();
    payload_length += transaction.t_value.length();
    payload_length += transaction.data.length();
    //EIP-155用に3フィールドを準備
    if let Some(cid) = chain_id {
        payload_length += cid.length();
        payload_length += 0u64.length();
        payload_length += 0u64.length();
    }
    // 2. バッファを確保し、リストのヘッダーを書き込む
    let mut out = BytesMut::with_capacity(payload_length + 10); // ヘッダー分少し余分に確保
    Header {
        list: true,
        payload_length,
    }
    .encode(&mut out);
    transaction.t_nonce.encode(&mut out);
    transaction.t_price.encode(&mut out);
    transaction.t_gas_limit.encode(&mut out);
    to_slice.encode(&mut out);
    transaction.t_value.encode(&mut out);
    transaction.data.encode(&mut out);

    if let Some(cid) = chain_id {
        cid.encode(&mut out);
        0u64.encode(&mut out);
        0u64.encode(&mut out);
    }
    let rlp_encoded = out.freeze();
    //4. Keccak256でハッシュ化して32バイトのh(T)を得る
    let mut hasher = Keccak256::new();
    hasher.update(&rlp_encoded);
    let tx_hash_bytes: [u8; 32] = hasher.finalize().into();
    // --- 公開鍵のリカバリ部分 ---
    let message = Message::from_digest(tx_hash_bytes);
    // 【解決策6】 `to_big_endian` の代わりに `to_be_bytes::<32>()` を使う
    let mut sig_bytes = [0u8; 64];
    sig_bytes[0..32].copy_from_slice(&transaction.t_r.to_be_bytes::<32>());
    sig_bytes[32..64].copy_from_slice(&transaction.t_s.to_be_bytes::<32>());
    let Ok(signature) = RecoverableSignature::from_compact(&sig_bytes, recovery_id) else {
        tracing::warn!("Invalid signature");
        return None;
    };
    let secp = Secp256k1::verification_only();
    // 【解決策7】 最新版では `&message` ではなく `message` (値渡し) にする
    let Ok(public_key) = secp.recover_ecdsa(message, &signature) else {
        tracing::warn!("Failed to recover public key");
        return None;
    };
    // あとは前回のコードと同じようにアドレスを抽出！
    let uncompressed_pubkey = public_key.serialize_uncompressed();
    let pubkey_hash = Keccak256::digest(&uncompressed_pubkey[1..65]);
    let mut sender_address = [0u8; 20];
    sender_address.copy_from_slice(&pubkey_hash[12..32]);
    let sender_address = Address::new(sender_address);
    return Some(sender_address);
}

//ブロックのBloom FilterとFilter条件を照らし合わせる
pub fn is_bloom_match(bloom: &Bloom, filter: &Filter) -> bool {
    // 1. アドレスのチェック (FilterSetはOptionではないので直接ループを回す)
    let mut addr_empty = true;
    let mut addr_matched = false;
    for addr in filter.address.clone().into_iter() {
        addr_empty = false;
        if bloom.contains_input(BloomInput::Raw(addr.as_slice())) {
            addr_matched = true;
        }
    }
    // フィルターが指定されているのに、一つもBloomに無ければスキップ
    if !addr_empty && !addr_matched {
        return false;
    }

    // 2. トピックのチェック
    for topic_set in filter.topics.clone().into_iter() {
        let mut topic_empty = true;
        let mut topic_matched = false;
        for topic in topic_set.into_iter() {
            topic_empty = false;
            if bloom.contains_input(BloomInput::Raw(topic.as_slice())) {
                topic_matched = true;
            }
        }
        if !topic_empty && !topic_matched {
            return false;
        }
    }

    true
}

//ログの厳密チェック (Exact Match)
pub fn is_exact_match(log: &PrimitiveLog, filter: &Filter) -> bool {
    // 1. アドレス照合
    let mut addr_empty = true;
    let mut addr_matched = false;
    for addr in filter.address.clone().into_iter() {
        addr_empty = false;
        if log.address == addr {
            addr_matched = true;
        }
    }
    if !addr_empty && !addr_matched {
        return false;
    }

    // 2. トピック照合 (AND / OR)
    for (i, topic_set) in filter.topics.clone().into_iter().enumerate() {
        let mut topic_empty = true;
        let mut topic_matched = false;
        for topic in topic_set.into_iter() {
            topic_empty = false;
            // ログの i 番目のトピックと比較
            if let Some(log_topic) = log.topics().get(i) {
                if topic == *log_topic {
                    topic_matched = true;
                }
            }
        }
        if !topic_empty && !topic_matched {
            return false;
        }
    }

    true
}

// 内部用のログ(PrimitiveLog)を、RPCレスポンス用のログ構造体に変換する
pub fn format_to_rpc_log(
    block_number: u64,
    block_hash: B256,
    transaction_hash: B256,
    transaction_index: u64,
    log_index: u64,
    log: &PrimitiveLog,
) -> alloy_rpc_types::Log {
    alloy_rpc_types::Log {
        inner: log.clone(),
        block_hash: Some(block_hash),
        block_number: Some(block_number),
        transaction_hash: Some(transaction_hash),
        transaction_index: Some(transaction_index),
        log_index: Some(log_index),
        removed: false,
        block_timestamp: None,
    }
}
