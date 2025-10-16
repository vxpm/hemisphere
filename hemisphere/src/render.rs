//! Renderer interface.

use crate::system::gpu::{
    VertexAttributes,
    command::attributes::Rgba,
    environment::{StageAlpha, StageColor},
};
use glam::Mat4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct TevStage {
    pub color: StageColor,
    pub alpha: StageAlpha,
}

pub enum Action {
    SetViewport(Viewport),
    SetClearColor(Rgba),
    SetProjectionMatrix(Mat4),
    SetTevStages(Vec<TevStage>),
    DrawTriangles(Vec<VertexAttributes>),
    DrawQuads(Vec<VertexAttributes>),
    Flush,
}

pub trait Renderer: Send + Sync {
    fn exec(&mut self, action: Action);
}
