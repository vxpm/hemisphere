import package::tev;

struct VertexMatrices {
    position: mat4x4f,
    textures: array<mat4x4f, 8>,
};

// Group 0: Vertex stage configuration
@group(0) @binding(0) var<uniform> matrices: VertexMatrices;

// Group 1: Fragment stage configuration
@group(1) @binding(0) var<uniform> tev_stages: tev::Stages;

// Group 2: Attributes
@group(2) @binding(0) var<storage> positions: array<vec3f>;
@group(2) @binding(1) var<storage> colors: array<vec4f>;

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return vec4f(0.0);
}
