//! Renderer interface.

use crate::system::gpu::{VertexAttributes, command::VertexAttributeSet};

pub struct Viewport {
    pub width: f32,
    pub height: f32,
}

pub enum Action {
    SetViewport(Viewport),
    SetVertexAttributes(VertexAttributeSet),
    DrawTriangle(Box<VertexAttributes>),
}

pub trait Renderer {
    fn exec(&mut self, action: Action);
}
