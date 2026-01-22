//! Vertex parsing module interface.
use std::mem::MaybeUninit;

use crate::system::gx::cmd::attributes::VertexAttributeTable;
use crate::system::gx::cmd::{Arrays, VertexAttributeStream, VertexDescriptor};
use crate::system::gx::xform::DefaultMatrices;
use crate::system::gx::{MatrixSet, Vertex};

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

/// An implementation of [`VertexModule`] that panics when used to parse a vertex stream.
pub struct NopVertexModule;

impl VertexModule for NopVertexModule {
    fn parse(
        &mut self,
        _: Ctx,
        _: &VertexDescriptor,
        _: &VertexAttributeTable,
        _: &VertexAttributeStream,
        _: &mut [MaybeUninit<Vertex>],
        _: &mut MatrixSet,
    ) {
        unimplemented!()
    }
}
