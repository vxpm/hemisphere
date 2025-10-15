//! Renderer interface.

use glam::Mat4;

use crate::system::gpu::{
    VertexAttributes,
    command::{VertexAttributeSet, attributes::Rgba},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

pub enum Action {
    SetViewport(Viewport),
    SetClearColor(Rgba),
    SetProjectionMatrix(Mat4),
    SetVertexAttributes(VertexAttributeSet),
    DrawTriangle(Vec<VertexAttributes>),
}

pub trait Renderer: Send + Sync {
    fn exec(&mut self, action: Action);
}
