use cranelift::{codegen::ir, prelude::isa::TargetIsa};
use fjall::{Database, KeyspaceCreateOptions};
use std::{
    hash::{Hash, Hasher},
    path::Path,
};
use twox_hash::XxHash3_128;
use zerocopy::IntoBytes;

struct Hash128(XxHash3_128);

impl Hasher for Hash128 {
    fn finish(&self) -> u64 {
        unimplemented!()
    }

    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FuncHash(u128);

impl FuncHash {
    pub fn new(isa: &dyn TargetIsa, stencil: &ir::function::FunctionStencil) -> Self {
        let mut hasher = Hash128(twox_hash::XxHash3_128::with_seed(0));
        isa.name().hash(&mut hasher);
        isa.triple().hash(&mut hasher);
        isa.isa_flags_hash_key().hash(&mut hasher);
        stencil.version_marker.hash(&mut hasher);
        stencil.signature.hash(&mut hasher);
        stencil.sized_stack_slots.hash(&mut hasher);
        stencil.dynamic_stack_slots.hash(&mut hasher);
        stencil.global_values.hash(&mut hasher);
        stencil.dfg.hash(&mut hasher);
        stencil.layout.hash(&mut hasher);
        Self(hasher.0.finish_128())
    }
}

pub struct Cache {
    db: Database,
    queries: u64,
    hits: u64,
    inserted: u16,
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
            inserted: 0,
            queries: 0,
            hits: 0,
        }
    }

    pub fn get(&mut self, hash: FuncHash) -> Option<Vec<u8>> {
        self.queries += 1;
        let artifacts = self
            .db
            .keyspace("artifacts", KeyspaceCreateOptions::default)
            .unwrap();

        let artifact = artifacts.get(hash.0.as_bytes()).unwrap();
        if artifact.is_some() {
            self.hits += 1;
            println!("{hash:?} OK !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
        } else {
            println!("{hash:?} missing");
            if hash.0 == 319813713416696475112911629735357558306 {
                panic!("what");
            }
        }

        println!("rate: {}", self.hits as f32 / self.queries as f32);

        artifact.map(|x| x.to_vec())
    }

    pub fn insert(&mut self, hash: FuncHash, code: &[u8]) {
        let artifacts = self
            .db
            .keyspace("artifacts", KeyspaceCreateOptions::default)
            .unwrap();

        artifacts.insert(hash.0.as_bytes(), code).unwrap();
        println!("inserted {hash:?}");

        self.inserted += 1;
        if self.inserted >= 256 {
            self.inserted = 0;
            self.db.persist(fjall::PersistMode::Buffer).unwrap();
        }
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        self.db.persist(fjall::PersistMode::SyncAll).unwrap();
    }
}
