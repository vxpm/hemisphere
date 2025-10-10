pub mod command;
pub mod environment;
pub mod transform;

use super::System;
use crate::system::gpu::command::{
    ArrayDescriptor, AttributeMode, VertexAttributeStream,
    attributes::{self, Attribute, AttributeDescriptor, Rgba},
};
use bitos::{
    bitos,
    integer::{UnsignedInt, u3, u4},
};
use common::{
    Primitive,
    bin::{BinReader, BinaryStream},
};
use glam::{Mat4, Vec2, Vec3};
use strum::FromRepr;
use thin_vec::ThinVec;
use zerocopy::IntoBytes;

/// An internal GX register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum Reg {
    GenMode = 0x00,
    GenFilter0 = 0x01,
    GenFilter1 = 0x02,
    GenFilter2 = 0x03,
    GenFilter3 = 0x04,
    BumpIMask = 0x0F,

    IndirectCmd0 = 0x10,
    IndirectCmd1 = 0x11,
    IndirectCmd2 = 0x12,
    IndirectCmd3 = 0x13,
    IndirectCmd4 = 0x14,
    IndirectCmd5 = 0x15,
    IndirectCmd6 = 0x16,
    IndirectCmd7 = 0x17,
    IndirectCmd8 = 0x18,
    IndirectCmd9 = 0x19,
    IndirectCmd10 = 0x1A,
    IndirectCmd11 = 0x1B,
    IndirectCmd12 = 0x1C,
    IndirectCmd13 = 0x1D,
    IndirectCmd14 = 0x1E,
    IndirectCmd15 = 0x1F,

    ScissorTopLeft = 0x20,
    ScissorBottomRight = 0x21,

    // Setup Unit and Rasterizer
    SetupLpSize = 0x22,
    SetupPerf = 0x23,
    RasterPerf = 0x24,
    RasterSs0 = 0x25,
    RasterSs1 = 0x26,

    RasterTexRef0 = 0x28,
    RasterTexRef1 = 0x29,
    RasterTexRef2 = 0x2A,
    RasterTexRef3 = 0x2B,
    RasterTexRef4 = 0x2C,
    RasterTexRef5 = 0x2D,
    RasterTexRef6 = 0x2E,
    RasterTexRef7 = 0x2F,

    SetupSsize0 = 0x30,
    SetupSsize1 = 0x32,
    SetupSsize2 = 0x34,
    SetupSsize3 = 0x36,
    SetupSsize4 = 0x38,
    SetupSsize5 = 0x3A,
    SetupSsize6 = 0x3C,
    SetupSsize7 = 0x3E,

    // Pixel Engine
    PixelZMode = 0x40,
    PixelMode0 = 0x41,
    PixelMode1 = 0x42,
    PixelControl = 0x43,
    PixelFieldMask = 0x44,
    PixelDone = 0x45,
    PixelRefresh = 0x46,
    PixelCopySrc = 0x49,
    PixelCopySrcSize = 0x4A,
    PixelCopyDstBase0 = 0x4B,
    PixelCopyDstBase1 = 0x4C,
    PixelCopyDstStride = 0x4D,
    PixelCopyScale = 0x4E,
    PixelCopyClearAr = 0x4F,
    PixelCopyClearGb = 0x50,
    PixelCopyClearZ = 0x51,
    PixelCopyCmd = 0x52,
    PixelCopyFilter0 = 0x53,
    PixelCopyFilter1 = 0x54,
    PixelXBound = 0x55,
    PixelYBound = 0x56,
    PixelPerfMode = 0x57,
    PixelChicken = 0x58,
    ScissorOffset = 0x59,

    // TX
    TxInvTags = 0x66,
    TxPerfMode = 0x67,
    TxFieldMode = 0x68,
    TxRefresh = 0x69,
    TxSetImage1I0 = 0x8C,
    TxSetImage1I1 = 0x8D,
    TxSetImage1I2 = 0x8E,
    TxSetImage1I3 = 0x8F,

    TxSetImage2I0 = 0x90,
    TxSetImage2I1 = 0x91,
    TxSetImage2I2 = 0x92,
    TxSetImage2I3 = 0x93,

    TxSetImage3I0 = 0x94,
    TxSetImage3I1 = 0x95,
    TxSetImage3I2 = 0x96,
    TxSetImage3I3 = 0x97,

    TxSetImage1I4 = 0xAC,
    TxSetImage1I5 = 0xAD,
    TxSetImage1I6 = 0xAE,
    TxSetImage1I7 = 0xAF,

    TxSetImage2I4 = 0xB0,
    TxSetImage2I5 = 0xB1,
    TxSetImage2I6 = 0xB2,
    TxSetImage2I7 = 0xB3,

    TxSetImage3I4 = 0xB4,
    TxSetImage3I5 = 0xB5,
    TxSetImage3I6 = 0xB6,
    TxSetImage3I7 = 0xB7,

    // TEV
    TevColor0 = 0xC0,
    TevAlpha0 = 0xC1,
    TevColor1 = 0xC2,
    TevAlpha1 = 0xC3,
    TevColor2 = 0xC4,
    TevAlpha2 = 0xC5,
    TevColor3 = 0xC6,
    TevAlpha3 = 0xC7,
    TevColor4 = 0xC8,
    TevAlpha4 = 0xC9,
    TevColor5 = 0xCA,
    TevAlpha5 = 0xCB,
    TevColor6 = 0xCC,
    TevAlpha6 = 0xCD,
    TevColor7 = 0xCE,
    TevAlpha7 = 0xCF,
    TevColor8 = 0xD0,
    TevAlpha8 = 0xD1,
    TevColor9 = 0xD2,
    TevAlpha9 = 0xD3,
    TevColor10 = 0xD4,
    TevAlpha10 = 0xD5,
    TevColor11 = 0xD6,
    TevAlpha11 = 0xD7,
    TevColor12 = 0xD8,
    TevAlpha12 = 0xD9,
    TevColor13 = 0xDA,
    TevAlpha13 = 0xDB,
    TevColor14 = 0xDC,
    TevAlpha14 = 0xDD,
    TevColor15 = 0xDE,
    TevAlpha15 = 0xDF,

    TevFogRange = 0xE8,
    TevFog0 = 0xEE,
    TevFog1 = 0xEF,
    TevFog2 = 0xF0,
    TevFog3 = 0xF1,
    TevFogColor = 0xF2,

    TevAlphaFunc = 0xF3,
    TevZ0 = 0xF4,
    TevZ1 = 0xF5,
    TevKSel0 = 0xF6,
    TevKSel1 = 0xF7,
    TevKSel2 = 0xF8,
    TevKSel3 = 0xF9,
    TevKSel4 = 0xFA,
    TevKSel5 = 0xFB,
    TevKSel6 = 0xFC,
    TevKSel7 = 0xFD,

    // BP
    BypassMask = 0xFE,
}

impl Reg {
    pub fn is_tev(&self) -> bool {
        matches!(
            self,
            Self::TevColor0
                | Self::TevAlpha0
                | Self::TevColor1
                | Self::TevAlpha1
                | Self::TevColor2
                | Self::TevAlpha2
                | Self::TevColor3
                | Self::TevAlpha3
                | Self::TevColor4
                | Self::TevAlpha4
                | Self::TevColor5
                | Self::TevAlpha5
                | Self::TevColor6
                | Self::TevAlpha6
                | Self::TevColor7
                | Self::TevAlpha7
                | Self::TevColor8
                | Self::TevAlpha8
                | Self::TevColor9
                | Self::TevAlpha9
                | Self::TevColor10
                | Self::TevAlpha10
                | Self::TevColor11
                | Self::TevAlpha11
                | Self::TevColor12
                | Self::TevAlpha12
                | Self::TevColor13
                | Self::TevAlpha13
                | Self::TevColor14
                | Self::TevAlpha14
                | Self::TevColor15
                | Self::TevAlpha15
        )
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CullingMode {
    #[default]
    None = 0b00,
    Negative = 0b01,
    Positive = 0b10,
    All = 0b11,
}

#[bitos(32)]
#[derive(Debug, Default)]
pub struct GenMode {
    #[bits(0..4)]
    pub tex_coords_count: u4,
    #[bits(4..8)]
    pub color_channels_count: u4,
    #[bits(9)]
    pub multisampling: bool,
    #[bits(10..14)]
    pub tev_stages_minus_one: u4,
    #[bits(14..16)]
    pub culling_mode: CullingMode,
    #[bits(16..19)]
    pub bumpmap_count: u3,
    #[bits(19)]
    pub z_freeze: bool,
}

/// Extracted vertex attributes.
#[derive(Debug)]
pub struct VertexAttributes {
    pub view_matrix: Option<ThinVec<Mat4>>,
    pub tex_matrix: [Option<ThinVec<Mat4>>; 8],
    pub position: Option<ThinVec<Vec3>>,
    pub normal: Option<ThinVec<Vec3>>,
    pub diffuse: Option<ThinVec<Rgba>>,
    pub specular: Option<ThinVec<Rgba>>,
    pub tex_uv: [Option<ThinVec<Vec2>>; 8],
}

/// GX subsystem
#[derive(Debug, Default)]
pub struct Gpu {
    pub command: command::Interface,
    pub transform: transform::Interface,
    pub environment: environment::Interface,
}

impl Gpu {
    fn set(&mut self, reg: Reg, value: u32) {
        match reg {
            Reg::GenMode => {
                let mode = GenMode::from_bits(value);
                self.environment.stages = mode.tev_stages_minus_one().value();
                self.environment.channels = mode.color_channels_count().value();
                tracing::debug!(?mode);
            }
            Reg::TevColor0 => {
                value.write_ne_bytes(self.environment.color_stages[0].as_mut_bytes());
            }
            Reg::TevAlpha0 => {
                value.write_ne_bytes(self.environment.alpha_stages[0].as_mut_bytes());
            }
            Reg::TevColor1 => {
                value.write_ne_bytes(self.environment.color_stages[1].as_mut_bytes());
            }
            Reg::TevAlpha1 => {
                value.write_ne_bytes(self.environment.alpha_stages[1].as_mut_bytes());
            }
            Reg::TevColor2 => {
                value.write_ne_bytes(self.environment.color_stages[2].as_mut_bytes());
            }
            Reg::TevAlpha2 => {
                value.write_ne_bytes(self.environment.alpha_stages[2].as_mut_bytes());
            }
            Reg::TevColor3 => {
                value.write_ne_bytes(self.environment.color_stages[3].as_mut_bytes());
            }
            Reg::TevAlpha3 => {
                value.write_ne_bytes(self.environment.alpha_stages[3].as_mut_bytes());
            }
            Reg::TevColor4 => {
                value.write_ne_bytes(self.environment.color_stages[4].as_mut_bytes());
            }
            Reg::TevAlpha4 => {
                value.write_ne_bytes(self.environment.alpha_stages[4].as_mut_bytes());
            }
            Reg::TevColor5 => {
                value.write_ne_bytes(self.environment.color_stages[5].as_mut_bytes());
            }
            Reg::TevAlpha5 => {
                value.write_ne_bytes(self.environment.alpha_stages[5].as_mut_bytes());
            }
            Reg::TevColor6 => {
                value.write_ne_bytes(self.environment.color_stages[6].as_mut_bytes());
            }
            Reg::TevAlpha6 => {
                value.write_ne_bytes(self.environment.alpha_stages[6].as_mut_bytes());
            }
            Reg::TevColor7 => {
                value.write_ne_bytes(self.environment.color_stages[7].as_mut_bytes());
            }
            Reg::TevAlpha7 => {
                value.write_ne_bytes(self.environment.alpha_stages[7].as_mut_bytes());
            }
            Reg::TevColor8 => {
                value.write_ne_bytes(self.environment.color_stages[8].as_mut_bytes());
            }
            Reg::TevAlpha8 => {
                value.write_ne_bytes(self.environment.alpha_stages[8].as_mut_bytes());
            }
            Reg::TevColor9 => {
                value.write_ne_bytes(self.environment.color_stages[9].as_mut_bytes());
            }
            Reg::TevAlpha9 => {
                value.write_ne_bytes(self.environment.alpha_stages[9].as_mut_bytes());
            }
            Reg::TevColor10 => {
                value.write_ne_bytes(self.environment.color_stages[10].as_mut_bytes());
            }
            Reg::TevAlpha10 => {
                value.write_ne_bytes(self.environment.alpha_stages[10].as_mut_bytes());
            }
            Reg::TevColor11 => {
                value.write_ne_bytes(self.environment.color_stages[11].as_mut_bytes());
            }
            Reg::TevAlpha11 => {
                value.write_ne_bytes(self.environment.alpha_stages[11].as_mut_bytes());
            }
            Reg::TevColor12 => {
                value.write_ne_bytes(self.environment.color_stages[12].as_mut_bytes());
            }
            Reg::TevAlpha12 => {
                value.write_ne_bytes(self.environment.alpha_stages[12].as_mut_bytes());
            }
            Reg::TevColor13 => {
                value.write_ne_bytes(self.environment.color_stages[13].as_mut_bytes());
            }
            Reg::TevAlpha13 => {
                value.write_ne_bytes(self.environment.alpha_stages[13].as_mut_bytes());
            }
            Reg::TevColor14 => {
                value.write_ne_bytes(self.environment.color_stages[14].as_mut_bytes());
            }
            Reg::TevAlpha14 => {
                value.write_ne_bytes(self.environment.alpha_stages[14].as_mut_bytes());
            }
            Reg::TevColor15 => {
                value.write_ne_bytes(self.environment.color_stages[15].as_mut_bytes());
            }
            Reg::TevAlpha15 => {
                value.write_ne_bytes(self.environment.alpha_stages[15].as_mut_bytes());
            }
            _ => tracing::warn!("unimplemented write to internal GX register {reg:?}"),
        }
    }
}

impl System {
    fn gx_read_attribute_from_array<D: AttributeDescriptor>(
        &mut self,
        descriptor: D,
        array: ArrayDescriptor,
        index: u16,
    ) -> D::Value {
        let base = array.address.value() as usize;
        let offset = array.stride.value() as usize * index as usize;
        let address = base + offset;
        let mut array = &self.mem.ram[address..];
        let mut reader = array.reader();
        descriptor.read(&mut reader).unwrap()
    }

    fn gx_read_attribute<A: Attribute>(
        &mut self,
        vat: usize,
        reader: &mut BinReader,
    ) -> Option<<A::Descriptor as AttributeDescriptor>::Value> {
        let mode = A::get_mode(&self.gpu.command.internal.vertex_descriptor);
        let descriptor = A::get_descriptor(&self.gpu.command.internal.vertex_attr_tables[vat]);

        match mode {
            AttributeMode::None => None,
            AttributeMode::Direct => Some(descriptor.read(reader).unwrap()),
            AttributeMode::Index8 => {
                let index = reader.read_be::<u8>().unwrap();
                let array = A::get_array(&self.gpu.command.internal.arrays).unwrap();
                Some(self.gx_read_attribute_from_array(descriptor, array, index as u16))
            }
            AttributeMode::Index16 => {
                let index = reader.read_be::<u16>().unwrap();
                let array = A::get_array(&self.gpu.command.internal.arrays).unwrap();
                Some(self.gx_read_attribute_from_array(descriptor, array, index))
            }
        }
    }

    pub fn gx_extract_attributes(&mut self, stream: VertexAttributeStream) -> VertexAttributes {
        let vcd = self.gpu.command.internal.vertex_descriptor.clone();
        let vat = stream.table_index();

        let mut position_data = vcd
            .position()
            .present()
            .then(|| ThinVec::with_capacity(stream.count() as usize));

        let mut diffuse_data = vcd
            .diffuse()
            .present()
            .then(|| ThinVec::with_capacity(stream.count() as usize));

        let mut data = stream.data();
        let mut reader = data.reader();
        for _ in 0..stream.count() {
            let value = self.gx_read_attribute::<attributes::Position>(vat, &mut reader);
            if let Some(value) = value {
                position_data.as_mut().unwrap().push(value);
            }

            let value = self.gx_read_attribute::<attributes::Diffuse>(vat, &mut reader);
            if let Some(value) = value {
                diffuse_data.as_mut().unwrap().push(value);
            }
        }

        VertexAttributes {
            view_matrix: None,
            tex_matrix: [const { None }; 8],
            position: position_data,
            diffuse: diffuse_data,
            normal: None,
            specular: None,
            tex_uv: [const { None }; 8],
        }
    }

    pub fn gx_draw_triangle(&mut self, stream: VertexAttributeStream) {
        let vcd = self.gpu.command.internal.vertex_descriptor.clone();
        let vat = self.gpu.command.internal.vertex_attr_tables[stream.table_index()].clone();

        tracing::debug!(?vcd);
        tracing::debug!(?vat);

        let attributes = self.gx_extract_attributes(stream);
        tracing::debug!(?attributes);

        let view_index = self.gpu.command.internal.mat_indices.view().value();
        tracing::debug!(%view_index);

        let view = self.gpu.transform.matrix(view_index);
        tracing::debug!(%view);

        let projection = self.gpu.transform.projection_matrix();
        tracing::debug!(%projection);

        // todo!()
    }
}
