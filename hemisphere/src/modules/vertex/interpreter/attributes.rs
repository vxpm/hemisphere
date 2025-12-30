use crate::system::gx::cmd::{
    ArrayDescriptor, Arrays, VertexDescriptor,
    attributes::{
        AttributeDescriptor, AttributeMode, ColorDescriptor, IndexDescriptor, NormalDescriptor,
        PositionDescriptor, TexCoordsDescriptor, VertexAttributeTable,
    },
};

/// A vertex attribute.
pub trait Attribute {
    /// Name of the attribute.
    const NAME: &'static str;
    /// The descriptor for this attribute.
    type Descriptor: AttributeDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode;
    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor;
    fn get_array(arrays: &Arrays) -> Option<ArrayDescriptor>;
}

pub struct PosMatrixIndex;

impl Attribute for PosMatrixIndex {
    const NAME: &'static str = "PosMatrixIndex";
    type Descriptor = IndexDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        if vcd.pos_mat_index() {
            AttributeMode::Direct
        } else {
            AttributeMode::None
        }
    }

    fn get_descriptor(_: &VertexAttributeTable) -> Self::Descriptor {
        IndexDescriptor
    }

    fn get_array(_: &Arrays) -> Option<ArrayDescriptor> {
        None
    }
}

pub struct TexMatrixIndex<const N: usize>;

impl<const N: usize> Attribute for TexMatrixIndex<N> {
    const NAME: &'static str = "TexMatrixIndex";
    type Descriptor = IndexDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        if vcd.tex_coord_mat_index_at(N).unwrap() {
            AttributeMode::Direct
        } else {
            AttributeMode::None
        }
    }

    fn get_descriptor(_: &VertexAttributeTable) -> Self::Descriptor {
        IndexDescriptor
    }

    fn get_array(_: &Arrays) -> Option<ArrayDescriptor> {
        None
    }
}

pub struct Position;

impl Attribute for Position {
    const NAME: &'static str = "Position";
    type Descriptor = PositionDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        vcd.position()
    }

    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor {
        vat.a.position()
    }

    fn get_array(arrays: &Arrays) -> Option<ArrayDescriptor> {
        Some(arrays.position)
    }
}

pub struct Normal;

impl Attribute for Normal {
    const NAME: &'static str = "Normal";
    type Descriptor = NormalDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        vcd.normal()
    }

    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor {
        vat.a.normal()
    }

    fn get_array(arrays: &Arrays) -> Option<ArrayDescriptor> {
        Some(arrays.normal)
    }
}

pub struct Diffuse;

impl Attribute for Diffuse {
    const NAME: &'static str = "Diffuse";
    type Descriptor = ColorDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        vcd.diffuse()
    }

    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor {
        vat.a.diffuse()
    }

    fn get_array(arrays: &Arrays) -> Option<ArrayDescriptor> {
        Some(arrays.diffuse)
    }
}

pub struct Specular;

impl Attribute for Specular {
    const NAME: &'static str = "Specular";
    type Descriptor = ColorDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        vcd.specular()
    }

    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor {
        vat.a.specular()
    }

    fn get_array(arrays: &Arrays) -> Option<ArrayDescriptor> {
        Some(arrays.specular)
    }
}

pub struct TexCoords<const N: usize>;

impl<const N: usize> Attribute for TexCoords<N> {
    const NAME: &'static str = "TexCoord";
    type Descriptor = TexCoordsDescriptor;

    fn get_mode(vcd: &VertexDescriptor) -> AttributeMode {
        vcd.tex_coord_at(N).unwrap()
    }

    fn get_descriptor(vat: &VertexAttributeTable) -> Self::Descriptor {
        vat.tex(N).unwrap()
    }

    fn get_array(arrays: &Arrays) -> Option<ArrayDescriptor> {
        Some(arrays.tex_coords[N])
    }
}
