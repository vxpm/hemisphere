use cranelift_codegen::incremental_cache::CacheKvStore;
use fjall::{Database, KeyspaceCreateOptions};
use std::borrow::Cow;
use std::fmt::Write;

pub struct RedbCache {
    db: Database,
    inserted: u16,
}

impl RedbCache {
    pub fn new(name: &str, clear: bool) -> Self {
        let dirs = directories::ProjectDirs::from("", "", "hemisphere").unwrap();
        let cache_dir = dirs.cache_dir();
        _ = std::fs::create_dir(cache_dir);

        let file_path = cache_dir.join(name);
        // let file = std::fs::File::options()
        //     .read(true)
        //     .write(true)
        //     .truncate(clear)
        //     .create(true)
        //     .open(file_path)
        //     .unwrap();

        let db = Database::builder(file_path)
            .journal_compression(fjall::CompressionType::Lz4)
            .manual_journal_persist(true)
            .open()
            .unwrap();
        Self { db, inserted: 0 }
    }
}

fn key_str(key: &[u8]) -> String {
    let mut s = String::with_capacity(key.len() * 2);
    for byte in key {
        write!(s, "{byte:02X}").unwrap();
    }

    s
}

impl CacheKvStore for RedbCache {
    fn get(&self, key: &[u8]) -> Option<Cow<'_, [u8]>> {
        let artifacts = self
            .db
            .keyspace("artifacts", KeyspaceCreateOptions::default)
            .unwrap();

        let artifact = artifacts.get(key).unwrap();

        let key = key_str(key);
        if artifact.is_some() {
            println!("{key} OK");
        } else {
            println!("{key} missing");
        }

        artifact.map(|x| Cow::Owned(x.to_vec()))
    }

    fn insert(&mut self, key: &[u8], val: Vec<u8>) {
        let artifacts = self
            .db
            .keyspace("artifacts", KeyspaceCreateOptions::default)
            .unwrap();

        artifacts.insert(key, val).unwrap();

        let key = key_str(key);
        println!("inserted {key}");

        self.inserted += 1;
        if self.inserted >= 256 {
            self.inserted = 0;
            self.db.persist(fjall::PersistMode::Buffer).unwrap();
        }
    }
}

impl Drop for RedbCache {
    fn drop(&mut self) {
        self.db.persist(fjall::PersistMode::SyncAll).unwrap();
    }
}
