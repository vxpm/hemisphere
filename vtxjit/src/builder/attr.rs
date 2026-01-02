use crate::builder::{MEMFLAGS, ParserBuilder};
use cranelift::{codegen::ir, prelude::InstBuilder};
use hemisphere::system::gx::{
    Vertex,
    cmd::{
        Arrays,
        attributes::{
            self, Attribute, AttributeDescriptor, ColorFormat, CoordsFormat, PositionKind,
        },
    },
};
use util::offset_of;

pub trait AttributeExt: Attribute {
    const ARRAY_OFFSET: usize;

    fn set_default(_parser: &mut ParserBuilder) {}
    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder) -> u32;
}

impl AttributeExt for attributes::PosMatrixIndex {
    const ARRAY_OFFSET: usize = usize::MAX;

    fn set_default(parser: &mut ParserBuilder) {
        parser.bd.ins().store(
            MEMFLAGS,
            parser.consts.default_pos,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, position_matrix) as i32,
        );
    }

    fn parse(_: &Self::Descriptor, parser: &mut ParserBuilder) -> u32 {
        let index = parser
            .bd
            .ins()
            .load(ir::types::I8, MEMFLAGS, parser.vars.data_ptr, 0);

        parser.include_matrix(false, index);

        parser.bd.ins().store(
            MEMFLAGS,
            index,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, position_matrix) as i32,
        );

        1
    }
}

impl AttributeExt for attributes::Position {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, position);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder) -> u32 {
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

        macro_rules! load_as_float {
            ($ty:expr, $offset:expr) => {{
                let value = parser
                    .bd
                    .ins()
                    .load($ty, MEMFLAGS, parser.vars.data_ptr, $offset);

                let value = if $ty.lane_type().is_float() {
                    value
                } else if signed {
                    parser
                        .bd
                        .ins()
                        .fcvt_from_sint(ir::types::F32.by($ty.lane_count()).unwrap(), value)
                } else {
                    parser
                        .bd
                        .ins()
                        .fcvt_from_uint(ir::types::F32.by($ty.lane_count()).unwrap(), value)
                };

                let shift = if $ty.lane_count() == 1 {
                    shift
                } else {
                    parser
                        .bd
                        .ins()
                        .splat(ir::types::F32.by($ty.lane_count()).unwrap(), shift)
                };

                parser.bd.ins().fmul(value, shift)
            }};
        }

        let xy_ty = ty.by(2).unwrap();
        let xy = load_as_float!(xy_ty, 0);

        parser.bd.ins().store(
            MEMFLAGS,
            xy,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, position) as i32,
        );

        let z = match desc.kind() {
            PositionKind::Vec2 => parser.bd.ins().f32const(0.0),
            PositionKind::Vec3 => load_as_float!(ty, xy_ty.bytes() as i32),
        };

        parser.bd.ins().store(
            MEMFLAGS,
            z,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, position.z) as i32,
        );

        desc.size()
    }
}

fn read_rgba(format: ColorFormat, parser: &mut ParserBuilder) -> ir::Value {
    let to_float = |parser: &mut ParserBuilder, rgba, max: u8| {
        let rgba = parser.bd.ins().fcvt_from_uint(ir::types::F32X4, rgba);
        let recip = parser.bd.ins().f32const(1.0 / (max as f32));
        let recip = parser.bd.ins().splat(ir::types::F32X4, recip);
        parser.bd.ins().fmul(rgba, recip)
    };

    match format {
        ColorFormat::Rgb565 => todo!(),
        ColorFormat::Rgb888 => {
            let r = parser
                .bd
                .ins()
                .load(ir::types::I8, MEMFLAGS, parser.vars.data_ptr, 0);
            let g = parser
                .bd
                .ins()
                .load(ir::types::I8, MEMFLAGS, parser.vars.data_ptr, 1);
            let b = parser
                .bd
                .ins()
                .load(ir::types::I8, MEMFLAGS, parser.vars.data_ptr, 2);
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
            let rgba = parser
                .bd
                .ins()
                .load(ir::types::I8X4, MEMFLAGS, parser.vars.data_ptr, 0);

            to_float(parser, rgba, 255)
        }
        _ => panic!("reserved color format"),
    }
}

impl AttributeExt for attributes::Diffuse {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, diffuse);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder) -> u32 {
        let rgba = read_rgba(desc.format(), parser);

        parser.bd.ins().store(
            MEMFLAGS,
            rgba,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, diffuse) as i32,
        );

        desc.size()
    }
}

impl AttributeExt for attributes::Specular {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, specular);

    fn parse(desc: &Self::Descriptor, parser: &mut ParserBuilder) -> u32 {
        let rgba = read_rgba(desc.format(), parser);

        parser.bd.ins().store(
            MEMFLAGS,
            rgba,
            parser.vars.vertex_ptr,
            offset_of!(Vertex, diffuse) as i32,
        );

        desc.size()
    }
}
