//! Renderer interface.

use crate::system::gx::{
    Topology, VertexAttributes,
    colors::{Abgr8, Rgba, Rgba8},
    pix::{BlendMode, BufferFormat, ConstantAlpha, DepthMode},
    tev::{AlphaFunction, Constant, StageOps, StageRefs},
    xf::{BaseTexGen, ChannelControl, Light},
};
use glam::Mat4;
use oneshot::Sender;
use ordered_float::OrderedFloat;

pub use oneshot;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub top_left_x: f32,
    pub top_left_y: f32,
    pub near_z: f32,
    pub far_z: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 640.0,
            height: 528.0,
            top_left_x: 0.0,
            top_left_y: 0.0,
            near_z: 0.0,
            far_z: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TexEnvStage {
    pub ops: StageOps,
    pub refs: StageRefs,
    pub color_const: Constant,
    pub alpha_const: Constant,
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
    SetFramebufferFormat(BufferFormat),
    SetViewport(Viewport),
    SetClearColor(Rgba),
    SetClearDepth(f32),
    SetDepthMode(DepthMode),
    SetBlendMode(BlendMode),
    SetConstantAlpha(ConstantAlpha),
    SetAlphaFunction(AlphaFunction),
    SetProjectionMatrix(Mat4),
    SetTexEnvConfig(TexEnvConfig),
    SetTexGenConfig(TexGenConfig),
    SetAmbient(u8, Abgr8),
    SetMaterial(u8, Abgr8),
    SetColorChannel(u8, ChannelControl),
    SetAlphaChannel(u8, ChannelControl),
    SetLight(u8, Light),
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
    ColorCopy {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        half: bool,
        clear: bool,
        response: Sender<Vec<Rgba8>>,
    },
    DepthCopy {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        half: bool,
        clear: bool,
        response: Sender<Vec<u32>>,
    },
    XfbCopy {
        clear: bool,
    },
}

pub trait Renderer: Send + Sync {
    fn exec(&mut self, action: Action);
}

pub struct NopRenderer;

impl Renderer for NopRenderer {
    fn exec(&mut self, _: Action) {}
}
