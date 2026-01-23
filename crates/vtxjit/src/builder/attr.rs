use cranelift::codegen::ir;
use cranelift::prelude::InstBuilder;
use lazuli::system::gx::Vertex;
use lazuli::system::gx::cmd::attributes::{
    self, Attribute, AttributeDescriptor, ColorFormat, ColorKind, CoordsFormat, PositionKind,
    TexCoordsKind,
};
use lazuli::system::gx::cmd::{ArrayDescriptor, Arrays};
use util::offset_of;
use zerocopy::IntoBytes;

use crate::builder::{MEMFLAGS, MEMFLAGS_READONLY, ParserBuilder};

fn split_i16(parser: &mut ParserBuilder, value: ir::Value) -> (ir::Value, ir::Value) {
    let low = parser.bd.ins().ireduce(ir::types::I8, value);
    let high = parser.bd.ins().ushr_imm(value, 8);
    let high = parser.bd.ins().ireduce(ir::types::I8, high);
    (low, high)
}

fn split_i32(parser: &mut ParserBuilder, value: ir::Value) -> (ir::Value, ir::Value) {
    let low = parser.bd.ins().ireduce(ir::types::I16, value);
    let high = parser.bd.ins().ushr_imm(value, 16);
    let high = parser.bd.ins().ireduce(ir::types::I16, high);
    (low, high)
}

fn split_i64(parser: &mut ParserBuilder, value: ir::Value) -> (ir::Value, ir::Value) {
    let low = parser.bd.ins().ireduce(ir::types::I32, value);
    let high = parser.bd.ins().ushr_imm(value, 32);
    let high = parser.bd.ins().ireduce(ir::types::I32, high);
    (low, high)
}

/// Parses a single vector coordinate encoded as U8/I8/U16/I16.
fn coord_int(
    parser: &mut ParserBuilder,
    ptr: ir::Value,
    ty: ir::Type,
    signed: bool,
    scale: ir::Value,
) -> ir::Value {
    // 01. load the integer value
    let value = parser.bd.ins().load(ty, MEMFLAGS_READONLY, ptr, 0);

    // 02. byteswap and extend
    let value = parser.bd.ins().bswap(value);
    let value = if signed {
        parser.bd.ins().sextend(ir::types::I32, value)
    } else {
        parser.bd.ins().uextend(ir::types::I32, value)
    };

    // 03. convert to F32
    let value = if signed {
        parser.bd.ins().fcvt_from_sint(ir::types::F32, value)
    } else {
        parser.bd.ins().fcvt_from_uint(ir::types::F32, value)
    };

    // 05. multiply by scale
    let value = parser.bd.ins().fmul(value, scale);
    value
}

/// Parses a single vector coordinate encoded as F32.
fn coord_float(parser: &mut ParserBuilder, ptr: ir::Value) -> ir::Value {
    // 01. load the value as an integer
    let value = parser
        .bd
        .ins()
        .load(ir::types::I32, MEMFLAGS_READONLY, ptr, 0);

    // 02. byteswap and bitcast
    let value = parser.bd.ins().bswap(value);
    let value = parser
        .bd
        .ins()
        .bitcast(ir::types::F32, ir::MemFlags::new(), value);

    value
}

/// Parses a vec2/vec3 with components encoded as U8/I8/U16/I16.
fn vec_int(
    parser: &mut ParserBuilder,
    ptr: ir::Value,
    ty: ir::Type,
    signed: bool,
    triplet: bool,
    scale: ir::Value,
) -> [ir::Value; 3] {
    // 01. load the integer values
    let pair_ty = match ty {
        ir::types::I8 => ir::types::I16,
        ir::types::I16 => ir::types::I32,
        _ => unreachable!(),
    };
    let pair = parser.bd.ins().load(pair_ty, MEMFLAGS_READONLY, ptr, 0);

    let (first, second) = if pair_ty == ir::types::I16 {
        split_i16(parser, pair)
    } else {
        split_i32(parser, pair)
    };

    let third = if triplet {
        parser
            .bd
            .ins()
            .load(ty, MEMFLAGS_READONLY, ptr, pair_ty.bytes() as i32)
    } else {
        parser.bd.ins().iconst(ty, 0)
    };

    // 02. extend them to I32
    let first = parser.bd.ins().uextend(ir::types::I32, first);
    let second = parser.bd.ins().uextend(ir::types::I32, second);
    let third = parser.bd.ins().uextend(ir::types::I32, third);

    // 03. put them in a I32X4
    let vector = parser.bd.ins().scalar_to_vector(ir::types::I32X4, first);
    let vector = parser.bd.ins().insertlane(vector, second, 1);
    let vector = parser.bd.ins().insertlane(vector, third, 2);

    // 04. BE -> LE
    let vector = if ty == ir::types::I16 {
        const ZEROED: u8 = 0xFF;
        const SHUFFLE_CONST: [u8; 16] = [
            1, 0, ZEROED, ZEROED, // lane 0 (first value)
            5, 4, ZEROED, ZEROED, // lane 1 (second value)
            9, 8, ZEROED, ZEROED, // lane 2 (third value)
            ZEROED, ZEROED, ZEROED, ZEROED, // lane 3 (dont care)
        ];

        let bytes = parser.bd.ins().bitcast(
            ir::types::I8X16,
            ir::MemFlags::new().with_endianness(ir::Endianness::Little),
            vector,
        );

        let shuffle_const = parser
            .bd
            .func
            .dfg
            .constants
            .insert(ir::ConstantData::from(SHUFFLE_CONST.as_bytes()));

        let shuffle_mask = parser.bd.ins().vconst(ir::types::I8X16, shuffle_const);
        let shuffled = parser.bd.ins().x86_pshufb(bytes, shuffle_mask);

        parser.bd.ins().bitcast(
            ir::types::I32X4,
            ir::MemFlags::new().with_endianness(ir::Endianness::Little),
            shuffled,
        )
    } else {
        vector
    };

    // 04. sign extend and convert to F32X4
    let vector = if signed {
        let left = parser.bd.ins().ishl_imm(vector, 32 - ty.bits() as i64);
        parser.bd.ins().sshr_imm(left, 32 - ty.bits() as i64)
    } else {
        vector
    };

    let vector = if signed {
        parser.bd.ins().fcvt_from_sint(ir::types::F32X4, vector)
    } else {
        parser.bd.ins().fcvt_from_uint(ir::types::F32X4, vector)
    };

    // 05. multiply by scale
    let scale = parser.bd.ins().splat(ir::types::F32X4, scale);
    let vector = parser.bd.ins().fmul(vector, scale);

    // 06. split it
    let first = parser.bd.ins().extractlane(vector, 0);
    let second = parser.bd.ins().extractlane(vector, 1);
    let third = parser.bd.ins().extractlane(vector, 2);

    [first, second, third]
}

/// Parses a vec2/vec3 with components encoded as F32.
fn vec_float(parser: &mut ParserBuilder, ptr: ir::Value, triplet: bool) -> [ir::Value; 3] {
    // 01. load the float values as I32s
    let pair = parser
        .bd
        .ins()
        .load(ir::types::I64, MEMFLAGS_READONLY, ptr, 0);
    let (first, second) = split_i64(parser, pair);
    let third = if triplet {
        parser.bd.ins().load(
            ir::types::I32,
            MEMFLAGS_READONLY,
            ptr,
            2 * size_of::<i32>() as i32,
        )
    } else {
        parser.bd.ins().iconst(ir::types::I32, 0)
    };

    // 02. put them in a I32X4
    let vector = parser.bd.ins().scalar_to_vector(ir::types::I32X4, first);
    let vector = parser.bd.ins().insertlane(vector, second, 1);
    let vector = parser.bd.ins().insertlane(vector, third, 2);

    // 03. BE -> LE
    const ZEROED: u8 = 0xFF;
    const SHUFFLE_CONST: [u8; 16] = [
        3, 2, 1, 0, // lane 0 (first value)
        7, 6, 5, 4, // lane 1 (second value)
        11, 10, 9, 8, // lane 2 (third value)
        ZEROED, ZEROED, ZEROED, ZEROED, // lane 3 (dont care)
    ];

    let bytes = parser.bd.ins().bitcast(
        ir::types::I8X16,
        ir::MemFlags::new().with_endianness(ir::Endianness::Little),
        vector,
    );

    let shuffle_const = parser
        .bd
        .func
        .dfg
        .constants
        .insert(ir::ConstantData::from(SHUFFLE_CONST.as_bytes()));

    let shuffle_mask = parser.bd.ins().vconst(ir::types::I8X16, shuffle_const);
    let shuffled = parser.bd.ins().x86_pshufb(bytes, shuffle_mask);

    // 04. convert to F32X4
    let vector = parser.bd.ins().bitcast(
        ir::types::F32X4,
        ir::MemFlags::new().with_endianness(ir::Endianness::Little),
        shuffled,
    );

    // 05. split it
    let first = parser.bd.ins().extractlane(vector, 0);
    let second = parser.bd.ins().extractlane(vector, 1);
    let third = parser.bd.ins().extractlane(vector, 2);

    [first, second, third]
}

fn rgba_vector(
    parser: &mut ParserBuilder,
    r: ir::Value,
    g: ir::Value,
    b: ir::Value,
    a: ir::Value,
) -> ir::Value {
    let (r, g, b, a) = if parser.bd.func.stencil.dfg.value_type(r) != ir::types::I32 {
        (
            parser.bd.ins().uextend(ir::types::I32, r),
            parser.bd.ins().uextend(ir::types::I32, g),
            parser.bd.ins().uextend(ir::types::I32, b),
            parser.bd.ins().uextend(ir::types::I32, a),
        )
    } else {
        (r, g, b, a)
    };

    let rgba = parser.bd.ins().scalar_to_vector(ir::types::I32X4, r);
    let rgba = parser.bd.ins().insertlane(rgba, g, 1);
    let rgba = parser.bd.ins().insertlane(rgba, b, 2);
    parser.bd.ins().insertlane(rgba, a, 3)
}

fn read_rgba(format: ColorFormat, parser: &mut ParserBuilder, ptr: ir::Value) -> ir::Value {
    let to_float = |parser: &mut ParserBuilder, rgba, recip| {
        let rgba = parser.bd.ins().fcvt_from_uint(ir::types::F32X4, rgba);
        parser.bd.ins().fmul(rgba, recip)
    };

    match format {
        ColorFormat::Rgb565 => {
            let value = parser
                .bd
                .ins()
                .load(ir::types::I16, MEMFLAGS_READONLY, ptr, 0);
            let value = parser.bd.ins().bswap(value);

            let r = parser.shift_mask(value, 0, 0x1F);
            let g = parser.shift_mask(value, 5, 0x3F);
            let b = parser.shift_mask(value, 11, 0x1F);
            let a = parser.bd.ins().iconst(ir::types::I16, 255);
            let rgba = rgba_vector(parser, r, g, b, a);

            const RECIP_CONST: [f32; 4] = [1.0 / 31.0, 1.0 / 63.0, 1.0 / 31.0, 1.0 / 255.0];
            let recip_const = parser
                .bd
                .func
                .dfg
                .constants
                .insert(ir::ConstantData::from(RECIP_CONST.as_bytes()));
            let recip = parser.bd.ins().vconst(ir::types::F32X4, recip_const);

            to_float(parser, rgba, recip)
        }
        ColorFormat::Rgb888 | ColorFormat::Rgb888x => {
            let rg = parser
                .bd
                .ins()
                .load(ir::types::I16, MEMFLAGS_READONLY, ptr, 0);
            let b = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 2);
            let a = parser.bd.ins().iconst(ir::types::I8, 255);
            let r = parser.bd.ins().band_imm(rg, 255);
            let g = parser.bd.ins().ushr_imm(rg, 8);

            let rgba = rgba_vector(parser, r, g, b, a);
            let recip = parser.bd.ins().f32const(1.0 / 255.0);
            let recip = parser.bd.ins().splat(ir::types::F32X4, recip);

            to_float(parser, rgba, recip)
        }
        ColorFormat::Rgba4444 => {
            let value = parser
                .bd
                .ins()
                .load(ir::types::I16, MEMFLAGS_READONLY, ptr, 0);
            let r = parser.shift_mask(value, 0, 15);
            let g = parser.shift_mask(value, 4, 15);
            let b = parser.shift_mask(value, 8, 15);
            let a = parser.shift_mask(value, 12, 15);

            let rgba = rgba_vector(parser, r, g, b, a);
            let recip = parser.bd.ins().f32const(1.0 / 15.0);
            let recip = parser.bd.ins().splat(ir::types::F32X4, recip);

            to_float(parser, rgba, recip)
        }
        ColorFormat::Rgba6666 => {
            let low = parser
                .bd
                .ins()
                .load(ir::types::I16, MEMFLAGS_READONLY, ptr, 0);
            let low = parser.bd.ins().uextend(ir::types::I32, low);

            let high = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 2);
            let high = parser.bd.ins().uextend(ir::types::I32, high);
            let high = parser.bd.ins().ishl_imm(high, 16);

            let value = parser.bd.ins().bor(low, high);
            let r = parser.shift_mask(value, 0, 63);
            let g = parser.shift_mask(value, 6, 63);
            let b = parser.shift_mask(value, 12, 63);
            let a = parser.shift_mask(value, 18, 63);

            let rgba = rgba_vector(parser, r, g, b, a);
            let recip = parser.bd.ins().f32const(1.0 / 63.0);
            let recip = parser.bd.ins().splat(ir::types::F32X4, recip);

            to_float(parser, rgba, recip)
        }
        ColorFormat::Rgba8888 => {
            let value = parser
                .bd
                .ins()
                .load(ir::types::I32, MEMFLAGS_READONLY, ptr, 0);
            let r = parser.shift_mask(value, 0, 255);
            let g = parser.shift_mask(value, 8, 255);
            let b = parser.shift_mask(value, 16, 255);
            let a = parser.shift_mask(value, 24, 255);

            let rgba = rgba_vector(parser, r, g, b, a);
            let recip = parser.bd.ins().f32const(1.0 / 255.0);
            let recip = parser.bd.ins().splat(ir::types::F32X4, recip);

            to_float(parser, rgba, recip)
        }
        _ => panic!("reserved color format"),
    }
}

pub trait AttributeExt: Attribute {
    const ARRAY_OFFSET: usize;

    fn set_default(_parser: &mut ParserBuilder) {}
    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32;
}

impl AttributeExt for attributes::PosMatrixIndex {
    const ARRAY_OFFSET: usize = usize::MAX;

    fn set_default(parser: &mut ParserBuilder) {
        parser.bd.ins().store(
            MEMFLAGS,
            parser.consts.default_pos,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, pos_norm_matrix) as i32,
        );
    }

    fn parse(_: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let index = parser
            .bd
            .ins()
            .load(ir::types::I8, MEMFLAGS_READONLY, ptr, 0);

        parser.include_matrix(false, index);
        parser.include_matrix(true, index);

        parser.bd.ins().store(
            MEMFLAGS,
            index,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, pos_norm_matrix) as i32,
        );

        1
    }
}

impl<const N: usize> AttributeExt for attributes::TexMatrixIndex<N> {
    const ARRAY_OFFSET: usize = usize::MAX;

    fn parse(_: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let index = parser
            .bd
            .ins()
            .load(ir::types::I8, MEMFLAGS_READONLY, ptr, 0);

        parser.include_matrix(false, index);
        parser.include_matrix(true, index);

        parser.bd.ins().store(
            MEMFLAGS,
            index,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, tex_coords_matrix) as i32 + size_of::<u8>() as i32 * N as i32,
        );

        1
    }
}

impl AttributeExt for attributes::Position {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, position);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let (ty, signed) = match desc.format() {
            CoordsFormat::U8 => (ir::types::I8, false),
            CoordsFormat::I8 => (ir::types::I8, true),
            CoordsFormat::U16 => (ir::types::I16, false),
            CoordsFormat::I16 => (ir::types::I16, true),
            CoordsFormat::F32 => (ir::types::F32, true),
            _ => panic!("reserved format"),
        };

        let scale = 1.0 / 2.0f32.powi(desc.shift().value() as i32);
        let scale = parser.bd.ins().f32const(scale);
        let triplet = desc.kind() == PositionKind::Vec3;

        let [x, y, z] = match ty {
            ir::types::I8 | ir::types::I16 => vec_int(parser, ptr, ty, signed, triplet, scale),
            _ => vec_float(parser, ptr, triplet),
        };

        parser.bd.ins().store(
            MEMFLAGS,
            x,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, position.x) as i32,
        );

        parser.bd.ins().store(
            MEMFLAGS,
            y,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, position.y) as i32,
        );

        parser.bd.ins().store(
            MEMFLAGS,
            z,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, position.z) as i32,
        );

        desc.size()
    }
}

impl AttributeExt for attributes::Normal {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, normal);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let (ty, signed) = match desc.format() {
            CoordsFormat::U8 => (ir::types::I8, false),
            CoordsFormat::I8 => (ir::types::I8, true),
            CoordsFormat::U16 => (ir::types::I16, false),
            CoordsFormat::I16 => (ir::types::I16, true),
            CoordsFormat::F32 => (ir::types::F32, true),
            _ => panic!("reserved format"),
        };

        let exp = if ty.bytes() == 1 { 6 } else { 14 };
        let scale = 1.0 / 2.0f32.powi(exp);
        let scale = parser.bd.ins().f32const(scale);

        let [x, y, z] = match ty {
            ir::types::I8 | ir::types::I16 => vec_int(parser, ptr, ty, signed, true, scale),
            _ => vec_float(parser, ptr, true),
        };

        parser.bd.ins().store(
            MEMFLAGS,
            x,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, normal.x) as i32,
        );

        parser.bd.ins().store(
            MEMFLAGS,
            y,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, normal.y) as i32,
        );

        parser.bd.ins().store(
            MEMFLAGS,
            z,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, normal.z) as i32,
        );

        desc.size()
    }
}

impl AttributeExt for attributes::Chan0 {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, chan0);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let rgba = read_rgba(desc.format(), parser, ptr);

        let rgba = if desc.kind() == ColorKind::Rgb {
            let max = parser.bd.ins().f32const(1.0);
            parser.bd.ins().insertlane(rgba, max, 3)
        } else {
            rgba
        };

        parser.bd.ins().store(
            MEMFLAGS,
            rgba,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, chan0) as i32,
        );

        desc.size()
    }
}

impl AttributeExt for attributes::Chan1 {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, chan1);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let rgba = read_rgba(desc.format(), parser, ptr);

        let rgba = if desc.kind() == ColorKind::Rgb {
            let max = parser.bd.ins().f32const(1.0);
            parser.bd.ins().insertlane(rgba, max, 3)
        } else {
            rgba
        };

        parser.bd.ins().store(
            MEMFLAGS,
            rgba,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, chan1) as i32,
        );

        desc.size()
    }
}

impl<const N: usize> AttributeExt for attributes::TexCoords<N> {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, tex_coords) + size_of::<ArrayDescriptor>() * N;

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let (ty, signed) = match desc.format() {
            CoordsFormat::U8 => (ir::types::I8, false),
            CoordsFormat::I8 => (ir::types::I8, true),
            CoordsFormat::U16 => (ir::types::I16, false),
            CoordsFormat::I16 => (ir::types::I16, true),
            CoordsFormat::F32 => (ir::types::F32, true),
            _ => panic!("reserved format"),
        };

        let scale = 1.0 / 2.0f32.powi(desc.shift().value() as i32);
        let scale = parser.bd.ins().f32const(scale);

        let [s, t] = match desc.kind() {
            TexCoordsKind::Vec1 => {
                let s = match ty {
                    ir::types::I8 | ir::types::I16 => coord_int(parser, ptr, ty, signed, scale),
                    _ => coord_float(parser, ptr),
                };
                let t = parser.bd.ins().f32const(0.0);

                [s, t]
            }
            TexCoordsKind::Vec2 => {
                let [s, t, _] = match ty {
                    ir::types::I8 | ir::types::I16 => {
                        vec_int(parser, ptr, ty, signed, false, scale)
                    }
                    _ => vec_float(parser, ptr, false),
                };

                [s, t]
            }
        };

        parser.bd.ins().store(
            MEMFLAGS,
            s,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, tex_coords) as i32 + N as i32 * size_of::<[f32; 2]>() as i32,
        );

        parser.bd.ins().store(
            MEMFLAGS,
            t,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, tex_coords) as i32
                + N as i32 * size_of::<[f32; 2]>() as i32
                + size_of::<f32>() as i32,
        );

        desc.size()
    }
}
