//! Renderer interface.

use crate::system::gpu::{
    Topology, VertexAttributes,
    command::attributes::Rgba,
    environment::{StageOps, StageRefs},
    pixel::{BlendMode, DepthMode},
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

#[derive(Debug, Clone)]
pub struct TevConfig {
    pub stages: Vec<TevStage>,
    pub constants: [Rgba; 4],
}

pub enum Action {
    SetViewport(Viewport),
    SetClearColor(Rgba),
    SetDepthMode(DepthMode),
    SetBlendMode(BlendMode),
    SetProjectionMatrix(Mat4),
    SetTevConfig(TevConfig),
    SetTexGens(Vec<TexGen>),
    LoadTexture {
        id: u32,
        width: u32,
        height: u32,
        data: Vec<Rgba8>,
    },
    SetTexture {
        index: usize,
        id: u32,
    },
    Draw(Topology, Vec<VertexAttributes>),
    EfbCopy {
        clear: bool,
    },
}

pub trait Renderer: Send + Sync {
    fn exec(&mut self, action: Action);
}
