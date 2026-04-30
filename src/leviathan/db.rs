use eth_trie::DB as EthTrieDB;
use rocksdb::{DB as RocksDB, Error as RocksError, Options, WriteBatch};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const CF_MPT: &str = "mpt_data";
pub const CF_CODE: &str = "code_data";

struct RocksDBInner {
    batch: WriteBatch,
    overlay: HashMap<Vec<u8>, Option<Vec<u8>>>,
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

        let cfs = vec![CF_MPT, CF_CODE];
        let db = RocksDB::open_cf(&opts, path, cfs).expect("RocksDBのオープンに失敗しました");

        Self {
            db: Arc::new(db),
            inner: Mutex::new(RocksDBInner {
                batch: WriteBatch::default(),
                overlay: HashMap::new(),
            }),
        }
    }


    pub fn insert_code(&self, code_hash: &[u8], code: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_CODE).unwrap();
        
        inner.batch.put_cf(&cf, code_hash, code);
        // overlayの型は Option<Vec<u8>> なので、Some で包んで入れる
        inner.overlay.insert(code_hash.to_vec(), Some(code.to_vec()));
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
        let current_batch = std::mem::replace(&mut inner.batch, WriteBatch::default());
        let result = self.db.write(current_batch);

        // SSD書き込み成功時に Overlay をクリアする
        if result.is_ok() {
            inner.overlay.clear();
        }

        result
    }
}
