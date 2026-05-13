use eth_trie::DB as EthTrieDB;
use rocksdb::{DB as RocksDB, Error as RocksError, Options, WriteBatch};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const CF_MPT: &str = "mpt_data";
pub const CF_CODE: &str = "code_data";
pub const CF_BLOCK_NUMBER: &str = "block_number";
pub const CF_BLOCK: &str = "block_data";
pub const CF_RECEIPT: &str = "receipt_data";
pub const CF_TX_LOOKUP: &str = "tx_lookup_data";
pub const BLOCK_NUMBER_KEY: &[u8] = &[1];

struct RocksDBInner {
    batch: WriteBatch,
    overlay: HashMap<Vec<u8>, Option<Vec<u8>>>,
    block_number: i64,
}

pub struct RocksDBWrapper {
    db: Arc<RocksDB>,
    inner: Mutex<RocksDBInner>,
}

impl RocksDBWrapper {
    pub fn new(path: &str) -> Self {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cfs = vec![
            CF_MPT,
            CF_CODE,
            CF_BLOCK_NUMBER,
            CF_BLOCK,
            CF_RECEIPT,
            CF_TX_LOOKUP,
        ];
        let db = RocksDB::open_cf(&opts, path, cfs).expect("RocksDBのオープンに失敗しました");

        Self {
            db: Arc::new(db),
            inner: Mutex::new(RocksDBInner {
                batch: WriteBatch::default(),
                overlay: HashMap::new(),
                block_number: 0,
            }),
        }
    }

    pub fn insert_code(&self, code_hash: &[u8], code: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_CODE).unwrap();

        inner.batch.put_cf(&cf, code_hash, code);
        // overlayの型は Option<Vec<u8>> なので、Some で包んで入れる
        inner
            .overlay
            .insert(code_hash.to_vec(), Some(code.to_vec()));
    }

    pub fn get_code(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        // 1. Overlayキャッシュを確認
        if let Some(cache_result) = inner.overlay.get(code_hash) {
            // cache_result は Option<Vec<u8>> なので、そのまま clone して返す
            return cache_result.clone();
        }
        // 2. キャッシュに無ければSSDを探す
        let cf = self.db.cf_handle(CF_CODE).unwrap();
        self.db.get_cf(&cf, code_hash).unwrap_or(None)
    }

    pub fn update_block_number(&self, new_number: i64, block_hash: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        //RocksDBWrapper.inner.block_numberを更新
        inner.block_number = new_number;
        //i64をu64に変換
        let Ok(new_number) = u64::try_from(new_number) else {
            panic!("block_numberが限界");
        };
        let block_number = new_number.to_be_bytes();
        let cf = self.db.cf_handle(CF_BLOCK_NUMBER).unwrap();
        inner.batch.put_cf(&cf, block_number, block_hash);
        inner
            .overlay
            .insert(block_number.to_vec(), Some(block_hash.to_vec()));
    }

    pub fn get_block_number(&self) -> Option<i64> {
        let inner = self.inner.lock().unwrap();
        Some(inner.block_number)
        
    }

    pub fn get_blockhash_from_index(&self, block_number: i64) -> Option<Vec<u8>> {
        //i64をu64に変換
        let Ok(block_number) = u64::try_from(block_number) else {
            panic!("block_numberが限界");
        };
        let block_number = block_number.to_be_bytes();
        let mut inner = self.inner.lock().unwrap();
        if let Some(cache_result) = inner.overlay.get(block_number.as_slice()) {
            return cache_result.clone();
        }
        let cf = self.db.cf_handle(CF_BLOCK_NUMBER).unwrap();
        self.db.get_cf(&cf, block_number).unwrap_or(None)
    }



    pub fn insert_receipt(&self, receipt_hash: &[u8], receipt_rlp: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_RECEIPT).unwrap();
        inner.batch.put_cf(&cf, receipt_hash, receipt_rlp);
        inner
            .overlay
            .insert(receipt_hash.to_vec(), Some(receipt_rlp.to_vec()));
    }

    pub fn get_receipt(&self, receipt_hash: &[u8]) -> Option<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        if let Some(cache_result) = inner.overlay.get(receipt_hash) {
            return cache_result.clone();
        }
        let cf = self.db.cf_handle(CF_RECEIPT).unwrap();
        self.db.get_cf(&cf, receipt_hash).unwrap_or(None)
    }

    pub fn insert_block(&self, block_hash: &[u8], block_rlp: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_BLOCK).unwrap();
        inner.batch.put_cf(&cf, block_hash, block_rlp);
        inner
            .overlay
            .insert(block_hash.to_vec(), Some(block_rlp.to_vec()));
    }

    pub fn get_block(&self, block_hash: &[u8]) -> Option<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        if let Some(cache_result) = inner.overlay.get(block_hash) {
            return cache_result.clone();
        }
        let cf = self.db.cf_handle(CF_BLOCK).unwrap();
        self.db.get_cf(&cf, block_hash).unwrap_or(None)
    }

    pub fn insert_txlookup(&self, key: &[u8], val: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_TX_LOOKUP).unwrap();
        inner.batch.put_cf(&cf, key, val);
        inner.overlay.insert(key.to_vec(), Some(val.to_vec()));
    }

    pub fn get_txlookup(&self, key: &[u8]) -> Option<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        if let Some(cache_result) = inner.overlay.get(key) {
            return cache_result.clone();
        }
        let cf = self.db.cf_handle(CF_TX_LOOKUP).unwrap();
        self.db.get_cf(&cf, key).unwrap_or(None)
    }
}

// --- MPT (eth_trie) 用メソッド ---

impl EthTrieDB for RocksDBWrapper {
    type Error = RocksError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let inner = self.inner.lock().unwrap();

        // 1. Overlayキャッシュを確認
        if let Some(cache_result) = inner.overlay.get(key) {
            // Some(Some(data)) -> 追加されたデータ
            // Some(None) -> 削除されたデータ(Tombstone)
            return Ok(cache_result.clone());
        }

        // 2. キャッシュに無ければSSDを探す
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        self.db.get_cf(&cf, key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();

        // バッチにPutし、キャッシュには Some(value) として記録
        inner.batch.put_cf(&cf, key, &value);
        inner.overlay.insert(key.to_vec(), Some(value));
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();

        // 🌟 バッチにDeleteし、キャッシュには None (Tombstone) として記録
        inner.batch.delete_cf(&cf, key);
        inner.overlay.insert(key.to_vec(), None);
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        let current_batch = std::mem::take(&mut inner.batch);
        let result = self.db.write(current_batch);

        // SSD書き込み成功時に Overlay をクリアする
        if result.is_ok() {
            inner.overlay.clear();
        }

        result
    }
}
