//! Renderer interface.

use crate::system::gpu::{
    Topology, VertexAttributes,
    command::attributes::Rgba,
    environment::{StageOps, StageRefs},
    pixel::{BlendMode, DepthMode},
    texture::Rgba8,
    transform::BaseTexGen,
};
use glam::Mat4;
use ordered_float::OrderedFloat;

/// Wrapper around a [`Mat4`] that allows hashing through [`OrderedFloat`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HashableMat4([OrderedFloat<f32>; 16]);

impl From<Mat4> for HashableMat4 {
    #[inline(always)]
    fn from(value: Mat4) -> Self {
        // SAFETY: this is safe because OrderedFloat is repr(transparent)
        Self(unsafe {
            std::mem::transmute::<[f32; 16], [OrderedFloat<f32>; 16]>(value.to_cols_array())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TexEnvStage {
    pub ops: StageOps,
    pub refs: StageRefs,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TexEnvConfig {
    pub stages: Vec<TexEnvStage>,
    pub constants: [Rgba; 4],
}

#[derive(Debug, Clone)]
pub struct TexGenStage {
    pub base: BaseTexGen,
    pub normalize: bool,
    pub post_matrix: Mat4,
}

impl PartialEq for TexGenStage {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
            && self.normalize == other.normalize
            && HashableMat4::from(self.post_matrix) == HashableMat4::from(other.post_matrix)
    }
}

impl Eq for TexGenStage {}

impl std::hash::Hash for TexGenStage {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.base.hash(state);
        self.normalize.hash(state);
        HashableMat4::from(self.post_matrix).hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TexGenConfig {
    pub stages: Vec<TexGenStage>,
}

pub enum Action {
    SetViewport(Viewport),
    SetClearColor(Rgba),
    SetDepthMode(DepthMode),
    SetBlendMode(BlendMode),
    SetProjectionMatrix(Mat4),
    SetTexEnvConfig(TexEnvConfig),
    SetTexGenConfig(TexGenConfig),
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

pub struct NopRenderer;

impl Renderer for NopRenderer {
    fn exec(&mut self, _: Action) {
        ()
    }
}
