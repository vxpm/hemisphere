//! Vertex parsing module interface.
mod interpreter;

use crate::system::gx::{
    MatrixSet, Vertex,
    cmd::{Arrays, VertexAttributeStream, VertexDescriptor, attributes::VertexAttributeTable},
    xform::DefaultMatrices,
};
use std::mem::MaybeUninit;

/// Trait for vertex parsing modules.
pub trait VertexModule: Send {
    fn parse(
        &mut self,
        ram: &[u8],
        vcd: &VertexDescriptor,
        vat: &VertexAttributeTable,
        arrays: &Arrays,
        default_matrices: &DefaultMatrices,
        stream: &VertexAttributeStream,
        vertices: &mut [MaybeUninit<Vertex>],
        matrix_set: &mut MatrixSet,
    );
}

/// The default vertex module.
pub use interpreter::Interpreter as InterpreterVertexModule;
