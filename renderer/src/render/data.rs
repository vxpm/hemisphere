use glam::{Vec2, Vec3};
use hemisphere::{
    render, system::gpu::command::attributes::Rgba, system::gpu::transform::TexGen as GpuTexGen,
};
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

#[derive(Debug, Clone, Immutable, IntoBytes, Default, PartialEq)]
#[repr(C)]
pub struct TevOpConfig {
    pub input_a: u32,
    pub input_b: u32,
    pub input_c: u32,
    pub input_d: u32,
    pub output: u32,

    pub sign: f32,
    pub bias: f32,
    pub scale: f32,
    pub clamp: u32,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default, PartialEq)]
#[repr(C)]
pub struct TevStageRefs {
    pub map: u32,
    pub coord: u32,
    pub color: u32,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default, PartialEq)]
#[repr(C)]
pub struct TevStage {
    pub color: TevOpConfig,
    pub alpha: TevOpConfig,
    pub refs: TevStageRefs,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default, PartialEq)]
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
            data.color.input_a = stage.ops.color.input_a() as u32;
            data.color.input_b = stage.ops.color.input_b() as u32;
            data.color.input_c = stage.ops.color.input_c() as u32;
            data.color.input_d = stage.ops.color.input_d() as u32;
            data.color.output = stage.ops.color.output() as u32;

            data.color.sign = if stage.ops.color.negate() { -1.0 } else { 1.0 };
            data.color.bias = stage.ops.color.bias().value();
            data.color.scale = stage.ops.color.scale().value();
            data.color.clamp = stage.ops.color.clamp() as u32;

            data.alpha.input_a = stage.ops.alpha.input_a() as u32;
            data.alpha.input_b = stage.ops.alpha.input_b() as u32;
            data.alpha.input_c = stage.ops.alpha.input_c() as u32;
            data.alpha.input_d = stage.ops.alpha.input_d() as u32;
            data.alpha.output = stage.ops.alpha.output() as u32;

            data.alpha.sign = if stage.ops.alpha.negate() { -1.0 } else { 1.0 };
            data.alpha.bias = stage.ops.alpha.bias().value();
            data.alpha.scale = stage.ops.alpha.scale().value();
            data.alpha.clamp = stage.ops.color.clamp() as u32;

            data.refs.map = stage.refs.map().value() as u32;
            data.refs.coord = stage.refs.coord().value() as u32;
            data.refs.color = stage.refs.color() as u32;
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

#[derive(Debug, Clone, Immutable, IntoBytes, Default, PartialEq)]
#[repr(C)]
pub struct TexGen {
    pub kind: u32,
    pub input_fmt: u32,
    pub output_fmt: u32,
    pub source: u32,
    pub emboss_source: u32,
    pub emboss_light: u32,
    pub _pad0: u32,
    pub _pad1: u32,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default, PartialEq)]
#[repr(C)]
pub struct TexGenConfig {
    pub count: u32,

    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,

    pub texgens: [TexGen; 8],
}

impl TexGenConfig {
    pub fn new(texgens: Vec<GpuTexGen>) -> Self {
        let count = texgens.len() as u32;
        let mut data = std::array::from_fn::<TexGen, 8, _>(|_| TexGen::default());

        for (texgen, data) in texgens.into_iter().zip(data.iter_mut()) {
            data.kind = texgen.kind() as u32;
            data.input_fmt = texgen.input_kind() as u32;
            data.output_fmt = texgen.output_kind() as u32;
            data.source = texgen.source() as u32;
            data.emboss_source = texgen.emboss_source().value() as u32;
            data.emboss_light = texgen.emboss_light().value() as u32;
        }

        Self {
            count,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            texgens: data,
        }
    }
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Config {
    pub tev: TevConfig,
    pub texgen: TexGenConfig,
}
