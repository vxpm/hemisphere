use cranelift_codegen::incremental_cache::CacheKvStore;
use fjall::{Database, KeyspaceCreateOptions};
use std::{borrow::Cow, cell::Cell, path::Path};

pub struct Cache {
    db: Database,
    queries: Cell<u64>,
    hits: Cell<u64>,
    pending: u16,
}

impl Cache {
    pub fn new(path: impl AsRef<Path>) -> Self {
        _ = std::fs::create_dir(&path);

        let db = Database::builder(&path)
            .journal_compression(fjall::CompressionType::Lz4)
            .manual_journal_persist(true)
            .open()
            .unwrap();

        Self {
            db,
            pending: 0,
            queries: Cell::new(0),
            hits: Cell::new(0),
        }
    }
}

impl CacheKvStore for Cache {
    fn get(&self, key: &[u8]) -> Option<std::borrow::Cow<'_, [u8]>> {
        self.queries.update(|x| x + 1);
        let artifacts = self
            .db
            .keyspace("artifacts", KeyspaceCreateOptions::default)
            .unwrap();

        let artifact = artifacts.get(key).unwrap();
        if artifact.is_some() {
            self.hits.update(|x| x + 1);
        }

        println!(
            "rate: {}",
            self.hits.get() as f32 / self.queries.get() as f32
        );

        artifact.map(|x| Cow::Owned(x.to_vec()))
    }

    fn insert(&mut self, key: &[u8], val: Vec<u8>) {
        let artifacts = self
            .db
            .keyspace("artifacts", KeyspaceCreateOptions::default)
            .unwrap();

        artifacts.insert(key, val).unwrap();

        self.pending += 1;
        if self.pending >= 256 {
            self.pending = 0;
            self.db.persist(fjall::PersistMode::Buffer).unwrap();
        }
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        self.db.persist(fjall::PersistMode::SyncAll).unwrap();
    }
}
