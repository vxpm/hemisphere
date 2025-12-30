//! Vertex parsing module interface.
mod interpreter;

use crate::system::gx::{
    MatrixMapping, Vertex,
    cmd::{Arrays, VertexAttributeStream, VertexDescriptor, attributes::VertexAttributeTable},
    xf::MatrixIndices,
};
use std::mem::MaybeUninit;

/// Trait for vertex parsing modules.
pub trait VertexModule: Send + Sync {
    fn parse(
        &mut self,
        ram: &[u8],
        vcd: &VertexDescriptor,
        vat: &VertexAttributeTable,
        arrays: &Arrays,
        default_matrices: &MatrixIndices,
        stream: &VertexAttributeStream,
        vertices: &mut [MaybeUninit<Vertex>],
        matrix_map: &mut Vec<MatrixMapping>,
    );
}

/// The default vertex module.
pub use interpreter::Interpreter as InterpreterVertexModule;
