use crate::builder::{MEMFLAGS, ParserBuilder};
use cranelift::{codegen::ir, prelude::InstBuilder};
use hemisphere::system::gx::{
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

pub trait AttributeExt: Attribute {
    const ARRAY_OFFSET: usize;

    fn set_default(_parser: &mut ParserBuilder) {}
    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32;
}

impl AttributeExt for attributes::PosMatrixIndex {
    const ARRAY_OFFSET: usize = usize::MAX;

    fn set_default(parser: &mut ParserBuilder) {
        parser.include_matrix(false, parser.consts.default_pos);
        parser.include_matrix(true, parser.consts.default_pos);

        parser.bd.ins().store(
            MEMFLAGS,
            parser.consts.default_pos,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, pos_norm_matrix) as i32,
        );
    }

    fn parse(_: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let index = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 0);
        let index = parser.bd.ins().uextend(ir::types::I16, index);
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

    fn set_default(parser: &mut ParserBuilder) {
        parser.include_matrix(false, parser.consts.default_tex[N]);
        parser.bd.ins().store(
            MEMFLAGS,
            parser.consts.default_tex[N],
            parser.vars.vertex_ptr,
            offset_of!(Vertex, tex_coords_matrix) as i32 + size_of::<u16>() as i32 * N as i32,
        );
    }

    fn parse(_: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let index = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 0);
        let index = parser.bd.ins().uextend(ir::types::I16, index);
        parser.include_matrix(false, index);
        parser.include_matrix(true, index);

        parser.bd.ins().store(
            MEMFLAGS,
            index,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, tex_coords_matrix) as i32 + size_of::<u16>() as i32 * N as i32,
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
                MEMFLAGS,
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
                MEMFLAGS,
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

fn read_rgba(format: ColorFormat, parser: &mut ParserBuilder, ptr: ir::Value) -> ir::Value {
    let to_float = |parser: &mut ParserBuilder, rgba, max: u8| {
        let rgba = parser.bd.ins().fcvt_from_uint(ir::types::F32X4, rgba);
        let recip = parser.bd.ins().f32const(1.0 / (max as f32));
        let recip = parser.bd.ins().splat(ir::types::F32X4, recip);
        parser.bd.ins().fmul(rgba, recip)
    };

    match format {
        ColorFormat::Rgb565 => todo!(),
        ColorFormat::Rgb888 => {
            let r = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 0);
            let g = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 1);
            let b = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 2);
            let a = parser.bd.ins().iconst(ir::types::I8, 255);

            let r = parser.bd.ins().uextend(ir::types::I32, r);
            let g = parser.bd.ins().uextend(ir::types::I32, g);
            let b = parser.bd.ins().uextend(ir::types::I32, b);
            let a = parser.bd.ins().uextend(ir::types::I32, a);

            let rgba = parser.bd.ins().scalar_to_vector(ir::types::I32X4, r);
            let rgba = parser.bd.ins().insertlane(rgba, g, 1);
            let rgba = parser.bd.ins().insertlane(rgba, b, 2);
            let rgba = parser.bd.ins().insertlane(rgba, a, 3);

            to_float(parser, rgba, 255)
        }
        ColorFormat::Rgb888x => todo!(),
        ColorFormat::Rgba4444 => todo!(),
        ColorFormat::Rgba6666 => todo!(),
        ColorFormat::Rgba8888 => {
            let r = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 0);
            let g = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 1);
            let b = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 2);
            let a = parser.bd.ins().load(ir::types::I8, MEMFLAGS, ptr, 3);

            let r = parser.bd.ins().uextend(ir::types::I32, r);
            let g = parser.bd.ins().uextend(ir::types::I32, g);
            let b = parser.bd.ins().uextend(ir::types::I32, b);
            let a = parser.bd.ins().uextend(ir::types::I32, a);

            let rgba = parser.bd.ins().scalar_to_vector(ir::types::I32X4, r);
            let rgba = parser.bd.ins().insertlane(rgba, g, 1);
            let rgba = parser.bd.ins().insertlane(rgba, b, 2);
            let rgba = parser.bd.ins().insertlane(rgba, a, 3);

            to_float(parser, rgba, 255)
        }
        _ => panic!("reserved color format"),
    }
}

impl AttributeExt for attributes::Chan0 {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, chan0);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder, ptr: ir::Value) -> u32 {
        let rgba = read_rgba(desc.format(), parser, ptr);

        let rgba = if desc.kind() == ColorKind::Rgb {
            let max = parser.bd.ins().iconst(ir::types::I8, 255);
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
            let max = parser.bd.ins().iconst(ir::types::I8, 255);
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
                MEMFLAGS,
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
