import package::{tev, texgen};

alias MatIdx = u32;
alias ConfigIdx = u32;

// Rendering configuration
struct Config {
    tev: tev::Config,
    texgen: texgen::Config,
};

// A primitive vertex
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

// Primitives group
@group(0) @binding(0) var<storage> configs: array<Config>;
@group(0) @binding(1) var<storage> matrices: array<mat4x4f>;
@group(0) @binding(2) var<storage> vertices: array<Vertex>;

// Textures group
@group(1) @binding(0) var texture0: texture_2d<f32>;
@group(1) @binding(1) var sampler0: sampler;
@group(1) @binding(2) var texture1: texture_2d<f32>;
@group(1) @binding(3) var sampler1: sampler;
@group(1) @binding(4) var texture2: texture_2d<f32>;
@group(1) @binding(5) var sampler2: sampler;
@group(1) @binding(6) var texture3: texture_2d<f32>;
@group(1) @binding(7) var sampler3: sampler;

@group(1) @binding(8) var texture4: texture_2d<f32>;
@group(1) @binding(9) var sampler4: sampler;
@group(1) @binding(10) var texture5: texture_2d<f32>;
@group(1) @binding(11) var sampler5: sampler;
@group(1) @binding(12) var texture6: texture_2d<f32>;
@group(1) @binding(13) var sampler6: sampler;
@group(1) @binding(14) var texture7: texture_2d<f32>;
@group(1) @binding(15) var sampler7: sampler;

struct VertexOutput {
    @builtin(position) clip: vec4f,
    @location(0) config: u32,
    @location(1) diffuse_color: vec4f,
    @location(2) specular_color: vec4f,
    @location(3) tex_coord0: vec3f,
    @location(4) tex_coord1: vec3f,
    @location(5) tex_coord2: vec3f,
    @location(6) tex_coord3: vec3f,
    @location(7) tex_coord4: vec3f,
    @location(8) tex_coord5: vec3f,
    @location(9) tex_coord6: vec3f,
    @location(10) tex_coord7: vec3f,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var out: VertexOutput;

    let vertex = vertices[index];

    let config = configs[vertex.config];
    out.config = vertex.config;
    
    let pos = vec4f(vertex.position, 1.0);
    let projection = matrices[vertex.projection];
    let view = matrices[vertex.position_mat];
    out.clip = projection * view * pos;
    out.clip.z += out.clip.w;
    out.clip.z /= 2.0;

    out.diffuse_color = vertex.diffuse;
    out.specular_color = vertex.specular;

    var tex_coords: array<vec3f, 8>;
    for (var i = 0u; i < config.texgen.count; i ++) {
        let texgen = config.texgen.texgens[i];
        let matrix = matrices[vertex.tex_coord_mat[i]];
        tex_coords[i] = texgen::compute(vertex, texgen, matrix);
    }

    out.tex_coord0 = tex_coords[0];
    out.tex_coord1 = tex_coords[1];
    out.tex_coord2 = tex_coords[2];
    out.tex_coord3 = tex_coords[3];
    out.tex_coord4 = tex_coords[4];
    out.tex_coord5 = tex_coords[5];
    out.tex_coord6 = tex_coords[6];
    out.tex_coord7 = tex_coords[7];

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let config = &configs[in.config];
    return tev::compute(config.tev, in.diffuse_color, in.specular_color, in.tex_coord0.xy);
}
