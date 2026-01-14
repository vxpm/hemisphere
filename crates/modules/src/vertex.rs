use lazuli::{
    modules::vertex::{Ctx, VertexModule},
    stream::{BinReader, BinaryStream},
    system::gx::{
        MatrixId, MatrixSet, Vertex,
        cmd::{
            ArrayDescriptor, VertexAttributeStream, VertexDescriptor,
            attributes::{
                self, Attribute, AttributeDescriptor, AttributeMode, VertexAttributeTable,
            },
        },
        glam::Vec2,
    },
};
use seq_macro::seq;
use std::mem::MaybeUninit;

#[inline(always)]
fn read_attribute_from_array<D: AttributeDescriptor>(
    ram: &[u8],
    descriptor: &D,
    array: ArrayDescriptor,
    index: u16,
) -> D::Value {
    let base = array.address.value() as usize;
    let offset = array.stride as usize * index as usize;
    let address = base + offset;
    let mut array = &ram[address..];
    let mut reader = array.reader();
    descriptor.read(&mut reader).unwrap()
}

#[inline(always)]
fn read_attribute<A: Attribute>(
    ctx: Ctx,
    vcd: &VertexDescriptor,
    vat: &VertexAttributeTable,
    reader: &mut BinReader,
) -> Option<<A::Descriptor as AttributeDescriptor>::Value> {
    let mode = A::get_mode(vcd);
    let descriptor = A::get_descriptor(vat);

    match mode {
        AttributeMode::None => None,
        AttributeMode::Direct => Some(
            descriptor
                .read(reader)
                .unwrap_or_else(|| panic!("failed to read {:?}", A::NAME)),
        ),
        AttributeMode::Index8 => {
            let index = reader.read_be::<u8>().unwrap() as u16;
            let array = A::get_array(ctx.arrays).unwrap();
            Some(read_attribute_from_array(
                ctx.ram,
                &descriptor,
                array,
                index,
            ))
        }
        AttributeMode::Index16 => {
            let index = reader.read_be::<u16>().unwrap();
            let array = A::get_array(ctx.arrays).unwrap();
            Some(read_attribute_from_array(
                ctx.ram,
                &descriptor,
                array,
                index,
            ))
        }
    }
}

pub struct InterpreterModule;

impl VertexModule for InterpreterModule {
    fn parse(
        &mut self,
        ctx: Ctx,
        vcd: &VertexDescriptor,
        vat: &VertexAttributeTable,
        stream: &VertexAttributeStream,
        vertices: &mut [MaybeUninit<Vertex>],
        matrix_set: &mut MatrixSet,
    ) {
        let default_pos_matrix_idx = ctx.default_matrices.view().value();

        let mut data = stream.data();
        let mut reader = data.reader();
        for i in 0..stream.count() {
            let pos_norm_matrix =
                read_attribute::<attributes::PosMatrixIndex>(ctx, vcd, vat, &mut reader)
                    .unwrap_or(default_pos_matrix_idx);

            let pos_norm_matrix = MatrixId::from_position_idx(pos_norm_matrix);
            matrix_set.include(pos_norm_matrix);
            matrix_set.include(pos_norm_matrix.normal());

            let mut tex_coords_matrix = [Default::default(); 8];
            seq! {
                N in 0..8 {
                    let default = ctx
                        .default_matrices
                        .tex_at(N)
                        .unwrap()
                        .value();

                    let tex_matrix_index =
                        read_attribute::<attributes::TexMatrixIndex<N>>(ctx, vcd, vat, &mut reader)
                            .unwrap_or(default);

                    tex_coords_matrix[N] = MatrixId::from_position_idx(tex_matrix_index);
                    matrix_set.include(tex_coords_matrix[N]);
                }
            }

            let position = read_attribute::<attributes::Position>(ctx, vcd, vat, &mut reader)
                .unwrap_or_default();

            let normal = read_attribute::<attributes::Normal>(ctx, vcd, vat, &mut reader)
                .unwrap_or_default();

            let chan0 =
                read_attribute::<attributes::Chan0>(ctx, vcd, vat, &mut reader).unwrap_or_default();

            let chan1 =
                read_attribute::<attributes::Chan1>(ctx, vcd, vat, &mut reader).unwrap_or_default();

            let mut tex_coords = [Vec2::ZERO; 8];
            seq! {
                N in 0..8 {
                    tex_coords[N] =
                        read_attribute::<attributes::TexCoords<N>>(ctx, vcd, vat, &mut reader)
                        .unwrap_or_default();
                }
            }

            vertices[i as usize].write(Vertex {
                position,
                normal,
                pos_norm_matrix,
                chan0,
                chan1,
                tex_coords,
                tex_coords_matrix,
            });
        }
    }
}
