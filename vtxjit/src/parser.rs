use hemisphere::system::gx::{
    MatrixSet, Vertex,
    cmd::{Arrays, VertexDescriptor, attributes::VertexAttributeTable},
    xf::MatrixIndices,
};
use jitalloc::{Allocation, Exec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Config {
    pub vcd: VertexDescriptor,
    pub vat: VertexAttributeTable,
}

impl Config {
    pub fn canonicalize(self) -> Self {
        // TODO: canonicalize
        self
    }
}

// ram, arrays, default matrices, data, vertices, matrix map, count
pub type ParserFn = extern "sysv64" fn(
    *const u8,
    *const Arrays,
    *const MatrixIndices,
    *const u8,
    *mut Vertex,
    *mut MatrixSet,
    u32,
);

pub struct VertexParser {
    code: Allocation<Exec>,
}

impl VertexParser {
    pub(crate) fn new(code: Allocation<Exec>) -> Self {
        Self { code }
    }

    pub(crate) fn as_ptr(&self) -> ParserFn {
        unsafe { std::mem::transmute(self.code.as_ptr().cast::<u8>()) }
    }
}
