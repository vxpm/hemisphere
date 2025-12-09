use crate::allocator::{Allocation, Allocator, Exec, ReadWrite};

pub struct Module {
    code_allocator: Allocator<Exec>,
    data_allocator: Allocator<ReadWrite>,
}

impl Module {
    pub fn new() -> Self {
        Self {
            code_allocator: Allocator::new(),
            data_allocator: Allocator::new(),
        }
    }

    pub fn allocate_code(&mut self, code: &[u8]) -> Allocation<Exec> {
        self.code_allocator.allocate(64, code)
    }

    pub fn allocate_data(&mut self, alignment: usize, length: usize) -> Allocation<ReadWrite> {
        self.data_allocator.allocate_uninit(alignment, length)
    }
}
