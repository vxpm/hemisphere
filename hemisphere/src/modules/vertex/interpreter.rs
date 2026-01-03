use crate::{
    modules::vertex::VertexModule,
    stream::{BinReader, BinaryStream},
    system::gx::{
        MatrixSet, Vertex,
        cmd::{
            ArrayDescriptor, Arrays, VertexAttributeStream, VertexDescriptor,
            attributes::{
                self, Attribute, AttributeDescriptor, AttributeMode, VertexAttributeTable,
            },
        },
        xform::DefaultMatrices,
    },
};
use glam::Vec2;
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
    ram: &[u8],
    vcd: &VertexDescriptor,
    vat: &VertexAttributeTable,
    arrays: &Arrays,
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
            let array = A::get_array(arrays).unwrap();
            Some(read_attribute_from_array(ram, &descriptor, array, index))
        }
        AttributeMode::Index16 => {
            let index = reader.read_be::<u16>().unwrap();
            let array = A::get_array(arrays).unwrap();
            Some(read_attribute_from_array(ram, &descriptor, array, index))
        }
    }
}

pub struct Interpreter;

impl VertexModule for Interpreter {
    fn parse(
        &mut self,
        ram: &[u8],
        vcd: &VertexDescriptor,
        vat: &VertexAttributeTable,
        arrays: &Arrays,
        default_matrices: &DefaultMatrices,
        stream: &VertexAttributeStream,
        vertices: &mut [MaybeUninit<Vertex>],
        matrix_set: &mut MatrixSet,
    ) {
        let default_pos_matrix_idx = default_matrices.view().value();

        let mut data = stream.data();
        let mut reader = data.reader();
        for i in 0..stream.count() {
            let pos_norm_matrix =
                read_attribute::<attributes::PosMatrixIndex>(ram, vcd, vat, arrays, &mut reader)
                    .unwrap_or(default_pos_matrix_idx) as u16;

            matrix_set.include(pos_norm_matrix);
            matrix_set.include(pos_norm_matrix + 256);

            let mut tex_coords_matrix = [0; 8];
            seq! {
                N in 0..8 {
                    let default = default_matrices
                        .tex_at(N)
                        .unwrap()
                        .value();

                    let tex_matrix_index =
                        read_attribute::<attributes::TexMatrixIndex<N>>(ram, vcd, vat, arrays, &mut reader)
                        .unwrap_or(default) as u16;
                    matrix_set.include(tex_matrix_index);
                    tex_coords_matrix[N] = tex_matrix_index;
                }
            }

            let position =
                read_attribute::<attributes::Position>(ram, vcd, vat, arrays, &mut reader)
                    .unwrap_or_default();

            let normal = read_attribute::<attributes::Normal>(ram, vcd, vat, arrays, &mut reader)
                .unwrap_or_default();

            let chan0 = read_attribute::<attributes::Chan0>(ram, vcd, vat, arrays, &mut reader)
                .unwrap_or_default();

            let chan1 = read_attribute::<attributes::Chan1>(ram, vcd, vat, arrays, &mut reader)
                .unwrap_or_default();

            let mut tex_coords = [Vec2::ZERO; 8];
            seq! {
                N in 0..8 {
                    tex_coords[N] =
                        read_attribute::<attributes::TexCoords<N>>(ram, vcd, vat, arrays, &mut reader)
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
