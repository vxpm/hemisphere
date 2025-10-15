//! Renderer interface.

use glam::Mat4;

use crate::system::gpu::{VertexAttributes, command::VertexAttributeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

pub enum Action {
    SetViewport(Viewport),
    SetPositionMatrix(Mat4),
    SetProjectionMatrix(Mat4),
    SetVertexAttributes(VertexAttributeSet),
    DrawTriangle(Box<VertexAttributes>),
}

pub trait Renderer: Send + Sync {
    fn exec(&mut self, action: Action);
}
