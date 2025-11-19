use memmap2::{Mmap, MmapOptions};

const PAGE_SIZE: usize = 4096;
const CHUNK_SIZE: usize = 256 * PAGE_SIZE;

pub struct Arena {
    previous: Vec<Mmap>,
    current: Option<Mmap>,
    offset: usize,
}

impl Arena {
    pub fn new() -> Self {
        let mapping = MmapOptions::new()
            .len(CHUNK_SIZE)
            .map_anon()
            .unwrap()
            .make_exec()
            .unwrap();

        Self {
            previous: vec![],
            current: Some(mapping),
            offset: 0,
        }
    }

    pub fn allocate(&mut self, data: &[u8]) -> *const [u8] {
        let mut mapping = self.current.take().unwrap();
        if mapping.len().saturating_sub(self.offset) < data.len() {
            let old = std::mem::replace(
                &mut mapping,
                MmapOptions::new()
                    .len(CHUNK_SIZE.max(data.len()))
                    .map_anon()
                    .unwrap()
                    .make_exec()
                    .unwrap(),
            );

            self.previous.push(old);
            self.offset = 0;
        }

        let ptr = &raw const mapping[self.offset..][..data.len()];
        let mut mapping = mapping.make_mut().unwrap();
        mapping[self.offset..][..data.len()].copy_from_slice(data);

        self.current = Some(mapping.make_exec().unwrap());
        self.offset = (self.offset + data.len()).next_multiple_of(16);

        ptr
    }
}
