//! Vertex parsing module interface.
mod interpreter;

use crate::system::gx::{
    MatrixSet, Vertex,
    cmd::{Arrays, VertexAttributeStream, VertexDescriptor, attributes::VertexAttributeTable},
    xform::DefaultMatrices,
};
use std::mem::MaybeUninit;

#[derive(Clone, Copy)]
pub struct Ctx<'ctx> {
    pub ram: &'ctx [u8],
    pub arrays: &'ctx Arrays,
    pub default_matrices: &'ctx DefaultMatrices,
}

/// Trait for vertex parsing modules.
pub trait VertexModule: Send {
    fn parse(
        &mut self,
        ctx: Ctx,
        vcd: &VertexDescriptor,
        vat: &VertexAttributeTable,
        stream: &VertexAttributeStream,
        vertices: &mut [MaybeUninit<Vertex>],
        matrix_set: &mut MatrixSet,
    );
}

/// The default vertex module.
pub use interpreter::Interpreter as InterpreterVertexModule;
