pub mod command;
pub mod transform;

use super::System;
use crate::system::gpu::command::{
    ArrayDescriptor, AttributeMode, VertexAttributeStream,
    attributes::{Attribute, Rgba},
};
use bitos::integer::UnsignedInt;
use common::bin::{BinReader, BinRingBuffer, BinaryStream};
use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use strum::FromRepr;

/// A bypass register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum BypassReg {
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
    RasterizerPerf = 0x24,
    RasterizerSs0 = 0x25,
    RasterizerSs1 = 0x26,

    RasterizerTexRef0 = 0x28,
    RasterizerTexRef1 = 0x29,
    RasterizerTexRef2 = 0x2A,
    RasterizerTexRef3 = 0x2B,
    RasterizerTexRef4 = 0x2C,
    RasterizerTexRef5 = 0x2D,
    RasterizerTexRef6 = 0x2E,
    RasterizerTexRef7 = 0x2F,

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

/// Extracted vertex attributes.
#[derive(Debug)]
pub struct VertexAttributes {
    pub position: Option<Vec<Vec3>>,
    pub diffuse: Option<Vec<Rgba>>,
}

/// GX subsystem
#[derive(Debug, Default)]
pub struct Gpu {
    pub command: command::Interface,
    pub command_queue: BinRingBuffer,
    pub transform: transform::Interface,
}

// fn guPerspective(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
//     let angle = (fovy * 0.50).to_radians();
//     let cot = 1.0 / angle.tan();
//     let tmp = 1.0 / (far - near);
//
//     Mat4::from_cols_array_2d(&[
//         [cot / aspect, 0.0, 0.0, 0.0],
//         [0.0, cot, 0.0, 0.0],
//         [0.0, 0.0, -near * tmp, -(far * near) * tmp],
//         [0.0, 0.0, -1.0, 0.0],
//     ])
// }

impl System {
    fn gx_read_attribute_value_from_array<A: Attribute>(
        &mut self,
        attr: A,
        array: ArrayDescriptor,
        index: u16,
    ) -> A::Value {
        let base = array.address.value() as usize;
        let offset = array.stride.value() as usize * index as usize;
        let address = base + offset;
        let mut array = &self.mem.ram[address..];
        let mut reader = array.reader();

        attr.read(&mut reader).unwrap()
    }

    fn gx_read_attribute_value<A: Attribute>(
        &mut self,
        mode: AttributeMode,
        attr: A,
        reader: &mut BinReader,
        array: ArrayDescriptor,
    ) -> Option<A::Value> {
        match mode {
            AttributeMode::None => None,
            AttributeMode::Direct => Some(attr.read(reader).unwrap()),
            AttributeMode::Index8 => {
                let index = reader.read_be::<u8>().unwrap();
                Some(self.gx_read_attribute_value_from_array(attr, array, index as u16))
            }
            AttributeMode::Index16 => {
                let index = reader.read_be::<u16>().unwrap();
                Some(self.gx_read_attribute_value_from_array(attr, array, index))
            }
        }
    }

    pub fn gx_extract_attributes(&mut self, stream: VertexAttributeStream) -> VertexAttributes {
        let vcd = self.gpu.command.internal.vertex_descriptor.clone();
        let vat = self.gpu.command.internal.vertex_attr_tables[stream.table_index()].clone();

        let mut position_data = vcd
            .position()
            .present()
            .then(|| Vec::with_capacity(stream.count() as usize));

        let mut diffuse_data = vcd
            .diffuse()
            .present()
            .then(|| Vec::with_capacity(stream.count() as usize));

        let mut data = stream.data();
        let mut reader = data.reader();
        for _ in 0..stream.count() {
            let position = self.gx_read_attribute_value(
                vcd.position(),
                vat.a.position(),
                &mut reader,
                self.gpu.command.internal.arrays.position.clone(),
            );

            if let Some(position) = position {
                position_data.as_mut().unwrap().push(position);
            }

            let diffuse = self.gx_read_attribute_value(
                vcd.diffuse(),
                vat.a.diffuse(),
                &mut reader,
                self.gpu.command.internal.arrays.diffuse.clone(),
            );

            if let Some(diffuse) = diffuse {
                diffuse_data.as_mut().unwrap().push(diffuse);
            }
        }

        VertexAttributes {
            position: position_data,
            diffuse: diffuse_data,
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
