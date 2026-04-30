use eth_trie::{DB as EthTrieDB, DBError};
use rocksdb::{DB as RocksDB, Options, WriteBatch};
use std::sync::{Arc, RwLock};

// カラムファミリ名の定義
pub const CF_MPT: &str = "mpt_data";
pub const CF_CODE: &str = "code_data"; // 将来のコード永続化用に予約

pub struct RocksDBWrapper {
    db: Arc<RocksDB>,
    // 内部可変性(Interior Mutability)を持たせ、&selfのままバッチを更新できるようにする
    batch: RwLock<WriteBatch>,
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
            batch: RwLock::new(WriteBatch::default()),
        }
    }
}

// eth_trie::DB トレイトの実装
impl EthTrieDB for RocksDBWrapper {
    // MPTからノードを読み込む（SSDから直接読み込み）
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DBError> {
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        self.db.get_cf(&cf, key).map_err(|e| DBError::Custom(e.to_string()))
    }

    // MPTに新しいノードを挿入する（メモリ上のWriteBatchに積むだけでSSDには書かない）
    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), DBError> {
        let mut batch = self.batch.write().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        
        batch.put_cf(&cf, key, value);
        Ok(())
    }

    // MPTからノードを削除する（メモリ上のWriteBatchに積むだけでSSDには書かない）
    fn remove(&self, key: &[u8]) -> Result<(), DBError> {
        let mut batch = self.batch.write().unwrap();
        let cf = self.db.cf_handle(CF_MPT).unwrap();
        
        batch.delete_cf(&cf, key);
        Ok(())
    }

    // CometBFTからのCommitシグナルを受け取った際に呼び出し、SSDへ一括書き込みを行う
    fn flush(&self) -> Result<(), DBError> {
        let mut batch = self.batch.write().unwrap();
        
        // 現在蓄積されているバッチを取り出し、メモリ上のバッチを空の初期状態にリセットする
        let current_batch = std::mem::replace(&mut *batch, WriteBatch::default());
        
        // アトミックにSSDへ書き込む（途中でクラッシュしてもデータは破損しない）
        self.db.write(current_batch).map_err(|e| DBError::Custom(e.to_string()))
    }
}
