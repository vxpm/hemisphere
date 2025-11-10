import package::uber::{MatIdx, Vertex};

alias OutputFormat = u32;
const OUT_FMT_VEC2: OutputFormat = 0x0;
const OUT_FMT_VEC3: OutputFormat = 0x1;

alias InputFormat = u32;
const IN_FMT_AB11: InputFormat = 0x0;
const IN_FMT_ABC1: InputFormat = 0x1;

alias Kind = u32;
const KIND_TRANSFORM: InputFormat = 0x0;
const KIND_EMBOSS: InputFormat = 0x1;
const KIND_COLOR_DIFFUSE: InputFormat = 0x2;
const KIND_COLOR_SPECULAR: InputFormat = 0x3;

alias Source = u32;
const SRC_POSITION: Source = 0x0;
const SRC_NORMAL: Source = 0x1;
const SRC_COLOR: Source = 0x2;
const SRC_BINORMAL_T: Source = 0x3;
const SRC_BINORMAL_B: Source = 0x4;
const SRC_TEXCOORD_0: Source = 0x5;
const SRC_TEXCOORD_1: Source = 0x6;
const SRC_TEXCOORD_2: Source = 0x7;
const SRC_TEXCOORD_3: Source = 0x8;
const SRC_TEXCOORD_4: Source = 0x9;
const SRC_TEXCOORD_5: Source = 0xA;
const SRC_TEXCOORD_6: Source = 0xB;
const SRC_TEXCOORD_7: Source = 0xC;

struct TexGen {
    kind: Kind,
    input_fmt: OutputFormat,
    output_fmt: OutputFormat,
    source: Source,
    emboss_source: u32,
    emboss_light: u32,
    normalize: u32,
    post_matrix_index: u32,
};

/// All TexGen configurations.
struct Config {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    texgens: array<TexGen, 8>,
};

fn compute(vertex: Vertex, texgen: TexGen, matrix: mat4x4f) -> vec3f {
    var source: vec3f;
    switch texgen.source {
        case SRC_POSITION: {
            source = vertex.position;
        }
        case SRC_NORMAL: {
            source = vertex.normal;
        }
        case SRC_TEXCOORD_0: {
            source = vec3f(vertex.tex_coord[0], 0.0);
        }
        case SRC_TEXCOORD_1: {
            source = vec3f(vertex.tex_coord[1], 0.0);
        }
        case SRC_TEXCOORD_2: {
            source = vec3f(vertex.tex_coord[2], 0.0);
        }
        case SRC_TEXCOORD_3: {
            source = vec3f(vertex.tex_coord[3], 0.0);
        }
        case SRC_TEXCOORD_4: {
            source = vec3f(vertex.tex_coord[4], 0.0);
        }
        case SRC_TEXCOORD_5: {
            source = vec3f(vertex.tex_coord[5], 0.0);
        }
        case SRC_TEXCOORD_6: {
            source = vec3f(vertex.tex_coord[6], 0.0);
        }
        case SRC_TEXCOORD_7: {
            source = vec3f(vertex.tex_coord[7], 0.0);
        }
        default: {
            // TODO
            source = vec3f(0.0);
        }
    }

    var input: vec4f;
    switch texgen.input_fmt {
        case IN_FMT_ABC1: {
            input = vec4f(source, 1.0);
        } 
        case IN_FMT_AB11: {
            input = vec4f(source.xy, 1.0, 1.0);
        }
        default: {
            input = vec4f(0.0);
        }
    }

    var computed: vec3f;
    switch texgen.kind {
        case KIND_TRANSFORM: {
            computed = (matrix * input).xyz;
        }
        default: {
            computed = vec3f(0.0);
        }
    }

    var reduced: vec3f;
    switch texgen.output_fmt {
        case OUT_FMT_VEC2: {
            reduced = vec3f(computed.xy, 0.0);
        }
        case OUT_FMT_VEC3: {
            reduced = computed;
        }
        default: {
            reduced = vec3f(0.0);
        }
    }

    var normalized: vec3f;
    if texgen.normalize != 0 {
        normalized = normalize(reduced);
    } else {
        normalized = reduced;   
    }

    var output: vec3f = (matrices[texgen.post_matrix_index] * vec4f(normalized, 1.0)).xyz;
    return output;
}