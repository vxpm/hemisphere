use cranelift::{
    codegen::ir,
    prelude::{FunctionBuilder, InstBuilder},
};
use hemisphere::system::gx::{
    Vertex,
    cmd::{
        Arrays, VertexDescriptor,
        attributes::{
            AttributeDescriptor, AttributeMode, CoordsFormat, PositionDescriptor, PositionKind,
            VertexAttributeTable,
        },
    },
};
use util::offset_of;

const MEMFLAGS: ir::MemFlags = ir::MemFlags::new().with_notrap().with_can_move();

pub trait AttributeDescriptorExt: AttributeDescriptor {
    fn parse(&self, bd: &mut FunctionBuilder, data_ptr: ir::Value, vertex_ptr: ir::Value) -> u32;
}

pub trait Attribute {
    const ARRAY_OFFSET: usize;

    type Descriptor: AttributeDescriptorExt;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode;
    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor;
}

impl AttributeDescriptorExt for PositionDescriptor {
    fn parse(&self, bd: &mut FunctionBuilder, data_ptr: ir::Value, vertex_ptr: ir::Value) -> u32 {
        let shift = 1.0 / 2.0f32.powi(self.shift().value() as i32);
        let shift = bd.ins().f32const(shift);

        let (ty, signed) = match self.format() {
            CoordsFormat::U8 => (ir::types::I8, false),
            CoordsFormat::I8 => (ir::types::I8, true),
            CoordsFormat::U16 => (ir::types::I16, false),
            CoordsFormat::I16 => (ir::types::I16, true),
            CoordsFormat::F32 => (ir::types::F32, true),
            _ => panic!("reserved format"),
        };

        macro_rules! load_as_float {
            ($ty:expr, $offset:expr) => {{
                let value = bd.ins().load($ty, MEMFLAGS, data_ptr, $offset);
                let value = if $ty.is_float() {
                    value
                } else if signed {
                    bd.ins()
                        .fcvt_from_sint(ir::types::F32.by($ty.lane_count()).unwrap(), value)
                } else {
                    bd.ins()
                        .fcvt_from_uint(ir::types::F32.by($ty.lane_count()).unwrap(), value)
                };

                bd.ins().fmul(value, shift)
            }};
        }

        let xy_ty = ty.by(2).unwrap();
        let xy = load_as_float!(xy_ty, 0);

        bd.ins().store(
            MEMFLAGS,
            xy,
            vertex_ptr,
            offset_of!(Vertex, position) as i32,
        );

        let z = match self.kind() {
            PositionKind::Vec2 => bd.ins().f32const(0.0),
            PositionKind::Vec3 => load_as_float!(ty, xy_ty.bytes() as i32),
        };

        bd.ins().store(
            MEMFLAGS,
            z,
            vertex_ptr,
            offset_of!(Vertex, position.z) as i32,
        );

        self.size()
    }
}

pub struct Position;

impl Attribute for Position {
    const ARRAY_OFFSET: usize = offset_of!(Arrays, position);

    type Descriptor = PositionDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        vcd.position()
    }

    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor {
        vat.a.position()
    }
}
