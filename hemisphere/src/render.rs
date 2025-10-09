//! Renderer interface.

use crate::system::gpu::VertexAttributes;

pub enum Action {
    DrawTriangle(VertexAttributes),
}

pub trait Renderer {
    fn exec(&mut self, action: Action);
}
