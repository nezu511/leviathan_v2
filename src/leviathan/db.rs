use eth_trie::DB as EthTrieDB;
use rocksdb::{DB as RocksDB, Error as RocksError, Options, WriteBatch};
use std::sync::{Arc, Mutex}; 

// カラムファミリ名の定義
pub const CF_MPT: &str = "mpt_data";
pub const CF_CODE: &str = "code_data";

pub struct RocksDBWrapper {
    db: Arc<RocksDB>,
    // WriteBatch は Sync ではないため、Mutex を使ってスレッドセーフにします
    batch: Mutex<WriteBatch>,
}

impl RocksDBWrapper {
    /// RocksDBインスタンスを初期化し、ラッパーを生成します
    pub fn new(path: &str) -> Self {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // 使用するカラムファミリを宣言
        let cfs = vec![CF_MPT, CF_CODE];
        
        // カラムファミリを有効にしてデータベースを開く
        let db = RocksDB::open_cf(&opts, path, cfs).expect("RocksDBのオープンに失敗しました");
        
        Self { 
            db: Arc::new(db),
            batch: Mutex::new(WriteBatch::default()), // Mutex::new になっています
        }
    }
}

// eth_trie::DB トレイトの実装
impl EthTrieDB for RocksDBWrapper {
    // 関連型 Error として、RocksDB のエラー型をそのまま使用する
    type Error = RocksError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        // RocksDBのget_cfは Result<Option<Vec<u8>>, rocksdb::Error> を返すのでそのまま返却
        self.db.get_cf(&cf, key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        // Mutex のロックを取得
        let mut batch = self.batch.lock().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        
        batch.put_cf(&cf, key, value);
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        // Mutex のロックを取得
        let mut batch = self.batch.lock().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        
        batch.delete_cf(&cf, key);
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        // Mutex のロックを取得
        let mut batch = self.batch.lock().unwrap();
        
        // 現在のバッチを取り出し、空のバッチと入れ替える
        let current_batch = std::mem::replace(&mut *batch, WriteBatch::default());
        
        // アトミックにSSDへ書き込み、Result をそのまま返す
        self.db.write(current_batch)
    }
}
