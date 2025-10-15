import package::{attr, tev};

// Contains the vertex matrices for the call.
struct Matrices {
    projection: mat4x4f,
    position: mat4x4f,
    // textures: array<mat4x4f, 8>,
};

// Group 0: Vertex stage configuration
@group(0) @binding(0) var<uniform> matrices: Matrices;

// Group 1: Fragment stage configuration
// @group(1) @binding(0) var<uniform> tev_stages: tev::Stages;

// Group 2: Attributes
@group(2) @binding(0) var<uniform> attributes: attr::Attributes;
@group(2) @binding(1) var<storage> positions: array<vec3f>;
@group(2) @binding(2) var<storage> colors: array<vec4f>;

struct VertexOutput {
    @builtin(position) clip: vec4f,
    @location(0) raster_color: vec4f,
};

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    
    let pos = vec4f(positions[index], 1.0);
    out.clip = matrices.projection * matrices.position * pos;
    out.clip.z += out.clip.w;
    out.clip.z /= 2.0;

    out.raster_color = colors[index];

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return in.raster_color;
}
