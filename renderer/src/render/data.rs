use glam::{Vec2, Vec3};
use hemisphere::{render, system::gpu::command::attributes::Rgba};
use zerocopy::{Immutable, IntoBytes};

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Vertex {
    pub config_idx: u32,
    pub projection_idx: u32,

    pub _pad0: u32,
    pub _pad1: u32,

    pub position: Vec3,
    pub position_mat_idx: u32,

    pub normal: Vec3,
    pub normal_mat_idx: u32,

    pub diffuse: Rgba,
    pub specular: Rgba,

    pub tex_coord: [Vec2; 8],
    pub tex_coord_mat_idx: [u32; 8],
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct TevStageInOut {
    pub input_a: u32,
    pub input_b: u32,
    pub input_c: u32,
    pub input_d: u32,
    pub output: u32,

    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct TevStageConfig {
    pub in_out: TevStageInOut,
    pub sign: f32,
    pub bias: f32,
    pub scale: f32,
    pub clamp: u32,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct TevStage {
    pub color: TevStageConfig,
    pub alpha: TevStageConfig,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct TevConfig {
    pub count: u32,

    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,

    pub stages: [TevStage; 16],
}

impl TevConfig {
    pub fn new(stages: Vec<render::TevStage>) -> Self {
        let count = stages.len() as u32;
        let mut data = std::array::from_fn::<TevStage, 16, _>(|_| TevStage::default());

        for (stage, data) in stages.into_iter().zip(data.iter_mut()) {
            data.color.in_out.input_a = stage.color.input_a() as u32;
            data.color.in_out.input_b = stage.color.input_b() as u32;
            data.color.in_out.input_c = stage.color.input_c() as u32;
            data.color.in_out.input_d = stage.color.input_d() as u32;
            data.color.in_out.output = stage.color.output() as u32;

            data.color.sign = if stage.color.negate() { -1.0 } else { 1.0 };
            data.color.bias = stage.color.bias().value();
            data.color.scale = stage.color.scale().value();
            data.color.clamp = 0;

            data.alpha.in_out.input_a = stage.alpha.input_a() as u32;
            data.alpha.in_out.input_b = stage.alpha.input_b() as u32;
            data.alpha.in_out.input_c = stage.alpha.input_c() as u32;
            data.alpha.in_out.input_d = stage.alpha.input_d() as u32;
            data.alpha.in_out.output = stage.alpha.output() as u32;

            data.alpha.sign = if stage.alpha.negate() { -1.0 } else { 1.0 };
            data.alpha.bias = stage.alpha.bias().value();
            data.alpha.scale = stage.alpha.scale().value();
            data.alpha.clamp = 0;
        }

        Self {
            count,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            stages: data,
        }
    }
}
