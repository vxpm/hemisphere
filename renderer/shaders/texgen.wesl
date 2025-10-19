import package::uber::{MatIdx};

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
    matrix: MatIdx,
    _pad0: u32,
};

/// All TexGen configurations.
struct Config {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    texgens: array<TexGen, 8>,
};