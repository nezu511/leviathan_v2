use eth_trie::DB as EthTrieDB;
use rocksdb::{DB as RocksDB, Error as RocksError, Options, WriteBatch};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const CF_MPT: &str = "mpt_data";
pub const CF_CODE: &str = "code_data";

struct RocksDBInner {
    batch: WriteBatch,
    // 🌟 テスト環境では、この overlay が実質的なメインDBとして機能する
    overlay: HashMap<Vec<u8>, Vec<u8>>,
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
}

impl EthTrieDB for RocksDBWrapper {
    type Error = RocksError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let inner = self.inner.lock().unwrap();

        // 🌟 常に Overlay キャッシュを最優先で確認！
        // これにより、さっき insert したばかりのノードを確実に見つける
        if let Some(value) = inner.overlay.get(key) {
            return Ok(Some(value.clone()));
        }

        // Overlay になければ、SSD(RocksDB本体)を探す
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        self.db.get_cf(&cf, key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();

        // バッチ(SSD書き込み予約)と Overlay(即時読み取り用) の両方に保存
        inner.batch.put_cf(&cf, key, &value);
        inner.overlay.insert(key.to_vec(), value);
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();

        inner.batch.delete_cf(&cf, key);
        inner.overlay.remove(key);
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        let current_batch = std::mem::replace(&mut inner.batch, WriteBatch::default());
        let result = self.db.write(current_batch);

        // 🌟 SSD書き込み成功時に Overlay をクリアする
        if result.is_ok() {
            inner.overlay.clear();
        }

        result
    }
}
