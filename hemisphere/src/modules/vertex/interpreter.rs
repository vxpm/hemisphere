mod attributes;

use crate::{
    modules::vertex::VertexModule,
    stream::{BinReader, BinaryStream},
    system::gx::{
        MatrixId, MatrixMapping, Vertex,
        cmd::{
            ArrayDescriptor, Arrays, VertexAttributeStream, VertexDescriptor,
            attributes::{AttributeDescriptor, AttributeMode, VertexAttributeTable},
        },
        xf::MatrixIndices,
    },
};
use attributes::Attribute;
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

impl Interpreter {
    fn get_matrix_id(
        &mut self,
        matrix_map: &mut Vec<MatrixMapping>,
        index: u8,
        normal: bool,
    ) -> MatrixId {
        let id = matrix_map
            .iter()
            .position(|m| m.normal == normal && m.index == index);

        if let Some(id) = id {
            id as MatrixId
        } else {
            matrix_map.push(MatrixMapping { index, normal });
            matrix_map.len() as MatrixId - 1
        }
    }
}

impl VertexModule for Interpreter {
    fn parse(
        &mut self,
        ram: &[u8],
        vcd: &VertexDescriptor,
        vat: &VertexAttributeTable,
        arrays: &Arrays,
        default_matrices: &MatrixIndices,
        stream: &VertexAttributeStream,
        vertices: &mut [MaybeUninit<Vertex>],
        matrix_map: &mut Vec<MatrixMapping>,
    ) {
        let default_pos_matrix_idx = default_matrices.view().value();

        let mut data = stream.data();
        let mut reader = data.reader();
        for i in 0..stream.count() {
            let position_matrix_index =
                read_attribute::<attributes::PosMatrixIndex>(ram, vcd, vat, arrays, &mut reader)
                    .unwrap_or(default_pos_matrix_idx);

            let position_matrix = self.get_matrix_id(matrix_map, position_matrix_index, false);
            let normal_matrix = self.get_matrix_id(matrix_map, position_matrix_index, true);

            let mut tex_coords_matrix = [0; 8];
            seq! {
                N in 0..8 {
                    let default = default_matrices
                        .tex_at(N)
                        .unwrap()
                        .value();

                    let tex_matrix_index =
                        read_attribute::<attributes::TexMatrixIndex<N>>(ram, vcd, vat, arrays, &mut reader)
                        .unwrap_or(default);

                    tex_coords_matrix[N] = self.get_matrix_id(matrix_map, tex_matrix_index, false);
                }
            }

            let position =
                read_attribute::<attributes::Position>(ram, vcd, vat, arrays, &mut reader)
                    .unwrap_or_default();

            let normal = read_attribute::<attributes::Normal>(ram, vcd, vat, arrays, &mut reader)
                .unwrap_or_default();

            let diffuse = read_attribute::<attributes::Diffuse>(ram, vcd, vat, arrays, &mut reader)
                .unwrap_or_default();

            let specular =
                read_attribute::<attributes::Specular>(ram, vcd, vat, arrays, &mut reader)
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
                position_matrix,
                normal,
                normal_matrix,
                diffuse,
                specular,
                tex_coords,
                tex_coords_matrix,
            });
        }
    }
}
