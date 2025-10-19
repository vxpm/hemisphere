//! Renderer interface.

use crate::system::gpu::{
    Topology, VertexAttributes,
    command::attributes::Rgba,
    environment::{StageOps, StageRefs},
    pixel::DepthMode,
    texture::Rgba8,
    transform::TexGen,
};
use glam::Mat4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct TevStage {
    pub ops: StageOps,
    pub refs: StageRefs,
}

pub enum Action {
    SetViewport(Viewport),
    SetClearColor(Rgba),
    SetDepthMode(DepthMode),
    SetProjectionMatrix(Mat4),
    SetTevStages(Vec<TevStage>),
    SetTexGens(Vec<TexGen>),
    SetTexture {
        index: usize,
        width: u32,
        height: u32,
        data: Vec<Rgba8>,
    },
    Draw(Topology, Vec<VertexAttributes>),
    EfbCopy {
        clear: bool,
    },
}

pub trait Renderer: Send + Sync {
    fn exec(&mut self, action: Action);
}
