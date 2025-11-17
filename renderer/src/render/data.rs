use glam::{Vec2, Vec3};
use hemisphere::system::gpu::colors::Rgba;
use zerocopy::{Immutable, IntoBytes};

#[derive(Debug, Clone, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub position_mat_idx: u32,

    pub normal: Vec3,
    pub normal_mat_idx: u32,

    pub diffuse: Rgba,
    pub specular: Rgba,

    pub tex_coord: [Vec2; 8],
    pub tex_coord_mat_idx: [u32; 8],

    pub projection_idx: u32,

    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}
