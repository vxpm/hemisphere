struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> uvs: vec4f;

var<private> POSITIONS: array<vec2f, 4> = array<vec2f, 4>(
    vec2f(-1.0, 1.0),
    vec2f(-1.0, -1.0),
    vec2f(1.0, 1.0),
    vec2f(1.0, -1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
) -> VertexOutput {
    let top_left = uvs.xy;
    let bottom_right = uvs.zw;

    let uvs = array<vec2f, 4>(
        top_left,
        vec2f(top_left.x, bottom_right.y),
        vec2f(bottom_right.x, top_left.y),
        bottom_right
    );

    return VertexOutput(vec4f(POSITIONS[index], 0.0, 1.0), uvs[index]);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(texture, texture_sampler, in.uv);
    return color;
}
