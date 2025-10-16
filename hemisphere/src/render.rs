//! Renderer interface.

use crate::system::gpu::{VertexAttributes, command::attributes::Rgba};
use glam::Mat4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

pub enum Action {
    SetViewport(Viewport),
    SetClearColor(Rgba),
    SetProjectionMatrix(Mat4),
    DrawTriangle(Vec<VertexAttributes>),
    Flush,
}

pub trait Renderer: Send + Sync {
    fn exec(&mut self, action: Action);
}
