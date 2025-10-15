import package::tev;

alias MatIdx = u32;
alias ConfigIdx = u32;

// Rendering configuration
struct Config {
    tev_stages: array<tev::Stage, 16>,
};

// A vertex primitive
struct Vertex {
    config: ConfigIdx, // 4 bytes
    projection: MatIdx, // 4 bytes

    // pad to 16 bytes
    _pad0: u32,
    _pad1: u32,

    position: vec3f, // 12 bytes
    position_mat: MatIdx, // 4 bytes

    normal: vec3f, // 12 bytes
    normal_mat: MatIdx, // 4 bytes

    diffuse: vec4f, // 16 bytes
    specular: vec4f, // 16 bytes

    tex_coord: array<vec2f, 8>, // 8 * 8 = 64 bytes
    tex_coord_mat: array<MatIdx, 8> // 4 * 8 = 32 bytes
};

@group(0) @binding(0) var<storage> configs: array<Config>;
@group(0) @binding(1) var<storage> matrices: array<mat4x4f>;
@group(0) @binding(2) var<storage> vertices: array<Vertex>;

struct VertexOutput {
    @builtin(position) clip: vec4f,
    @location(0) raster_color: vec4f,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    let vertex = vertices[index];
    let pos = vec4f(vertex.position, 1.0);
    let projection = matrices[vertex.projection];
    let view = matrices[vertex.position_mat];
    out.clip = projection * view * pos;
    out.clip.z += out.clip.w;
    out.clip.z /= 2.0;

    out.raster_color = vertex.diffuse;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return in.raster_color;
}
