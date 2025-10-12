struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
};

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

const vertices = array<vec2f, 4>(
    vec2f(-1.0, 1.0),
    vec2f(-1.0, -1.0),
    vec2f(1.0, 1.0),
    vec2f(1.0, -1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
) -> VertexOutput {
    return VertexOutput(vec4f(vertices[index], 0.0, 1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.3, 0.2, 0.1, 1.0);
}
