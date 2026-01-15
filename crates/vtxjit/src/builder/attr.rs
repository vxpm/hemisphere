use crate::builder::{MEMFLAGS, MEMFLAGS_READONLY, ParserBuilder};
use cranelift::{codegen::ir, prelude::InstBuilder};
use lazuli::system::gx::{
    Vertex,
    cmd::{
        ArrayDescriptor, Arrays,
        attributes::{
            self, Attribute, AttributeDescriptor, ColorFormat, ColorKind, CoordsFormat,
            PositionKind, TexCoordsKind,
        },
    },
};
use util::offset_of;
use zerocopy::IntoBytes;

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
        let shift = 1.0 / 2.0f32.powi(desc.shift().value() as i32);
        let shift = parser.bd.ins().f32const(shift);

        let (ty, signed) = match desc.format() {
            CoordsFormat::U8 => (ir::types::I8, false),
            CoordsFormat::I8 => (ir::types::I8, true),
            CoordsFormat::U16 => (ir::types::I16, false),
            CoordsFormat::I16 => (ir::types::I16, true),
            CoordsFormat::F32 => (ir::types::F32, true),
            _ => panic!("reserved format"),
        };

        let mut load_as_float = |index| {
            let value = parser.bd.ins().load(
                if ty.is_float() { ir::types::I32 } else { ty },
                MEMFLAGS_READONLY,
                ptr,
                index * ty.bytes() as i32,
            );

            let value = if ty.bytes() == 1 {
                value
            } else if ty.is_float() {
                let value = parser.bd.ins().bswap(value);
                parser
                    .bd
                    .ins()
                    .bitcast(ir::types::F32, ir::MemFlags::new(), value)
            } else {
                parser.bd.ins().bswap(value)
            };

            let value = if ty.is_float() {
                value
            } else if signed {
                parser.bd.ins().fcvt_from_sint(ir::types::F32, value)
            } else {
                parser.bd.ins().fcvt_from_uint(ir::types::F32, value)
            };

            if ty.is_float() {
                value
            } else {
                parser.bd.ins().fmul(value, shift)
            }
        };

        let x = load_as_float(0);
        let y = load_as_float(1);

        let z = match desc.kind() {
            PositionKind::Vec2 => parser.bd.ins().f32const(0.0),
            PositionKind::Vec3 => load_as_float(2),
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
        let shift = 1.0 / 2.0f32.powi(exp);
        let shift = parser.bd.ins().f32const(shift);

        let mut load_as_float = |index| {
            let value = parser.bd.ins().load(
                if ty.is_float() { ir::types::I32 } else { ty },
                MEMFLAGS_READONLY,
                ptr,
                index * ty.bytes() as i32,
            );

            let value = if ty.bytes() == 1 {
                value
            } else if ty.is_float() {
                let value = parser.bd.ins().bswap(value);
                parser
                    .bd
                    .ins()
                    .bitcast(ir::types::F32, ir::MemFlags::new(), value)
            } else {
                parser.bd.ins().bswap(value)
            };

            let value = if ty.is_float() {
                value
            } else if signed {
                parser.bd.ins().fcvt_from_sint(ir::types::F32, value)
            } else {
                parser.bd.ins().fcvt_from_uint(ir::types::F32, value)
            };

            if ty.is_float() {
                value
            } else {
                parser.bd.ins().fmul(value, shift)
            }
        };

        let x = load_as_float(0);
        let y = load_as_float(1);
        let z = load_as_float(2);

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
    let rgba = parser.bd.ins().insertlane(rgba, a, 3);

    rgba
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

            const SIMD_CONST: [f32; 4] = [1.0 / 31.0, 1.0 / 63.0, 1.0 / 31.0, 1.0 / 255.0];
            let recip_const = parser
                .bd
                .func
                .dfg
                .constants
                .insert(ir::ConstantData::from(SIMD_CONST.as_bytes()));
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
        let shift = 1.0 / 2.0f32.powi(desc.shift().value() as i32);
        let shift = parser.bd.ins().f32const(shift);

        let (ty, signed) = match desc.format() {
            CoordsFormat::U8 => (ir::types::I8, false),
            CoordsFormat::I8 => (ir::types::I8, true),
            CoordsFormat::U16 => (ir::types::I16, false),
            CoordsFormat::I16 => (ir::types::I16, true),
            CoordsFormat::F32 => (ir::types::F32, true),
            _ => panic!("reserved format"),
        };

        let mut load_as_float = |index| {
            let value = parser.bd.ins().load(
                if ty.is_float() { ir::types::I32 } else { ty },
                MEMFLAGS_READONLY,
                ptr,
                index * ty.bytes() as i32,
            );

            let value = if ty.bytes() == 1 {
                value
            } else if ty.is_float() {
                let value = parser.bd.ins().bswap(value);
                parser
                    .bd
                    .ins()
                    .bitcast(ir::types::F32, ir::MemFlags::new(), value)
            } else {
                parser.bd.ins().bswap(value)
            };

            let value = if ty.is_float() {
                value
            } else if signed {
                parser.bd.ins().fcvt_from_sint(ir::types::F32, value)
            } else {
                parser.bd.ins().fcvt_from_uint(ir::types::F32, value)
            };

            if ty.is_float() {
                value
            } else {
                parser.bd.ins().fmul(value, shift)
            }
        };

        let s = load_as_float(0);
        let t = match desc.kind() {
            TexCoordsKind::Vec1 => parser.bd.ins().f32const(0.0),
            TexCoordsKind::Vec2 => load_as_float(1),
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
