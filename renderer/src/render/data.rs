use glam::{Mat3, Mat4, Vec2, Vec3};
use hemisphere::system::gpu::colors::Rgba;
use zerocopy::{Immutable, IntoBytes};

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub config_idx: u32,
    pub normal: Vec3,
    pub _pad0: u32,

    pub projection_mat: Mat4,
    pub position_mat: Mat4,
    pub normal_mat: Mat3,

    pub _pad1: u32,
    pub _pad2: u32,
    pub _pad3: u32,

    pub diffuse: Rgba,
    pub specular: Rgba,

    pub tex_coord: [Vec2; 8],
    pub tex_coord_mat: [Mat4; 8],
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Light {
    pub color: Rgba,

    pub cos_attenuation: Vec3,
    pub _pad0: u32,

    pub dist_attenuation: Vec3,
    pub _pad1: u32,

    pub position: Vec3,
    pub _pad2: u32,

    pub direction: Vec3,
    pub _pad3: u32,
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Channel {
    pub material_from_vertex: u32,
    pub ambient_from_vertex: u32,
    pub lighting_enabled: u32,
    pub diffuse_attenuation: u32,
    pub attenuation: u32,
    pub spotlight: u32,
    pub light_mask: [u32; 8],
}

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Config {
    pub ambient: [Rgba; 2],
    pub material: [Rgba; 2],
    pub lights: [Light; 8],
    pub color_channels: [Channel; 2],
    pub alpha_channels: [Channel; 2],
}
