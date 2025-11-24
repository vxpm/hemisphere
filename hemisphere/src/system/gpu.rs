pub mod colors;
pub mod command;
pub mod environment;
pub mod pixel;
pub mod texture;
pub mod transform;

use crate::{
    Primitive, System,
    render::{Action, TexEnvConfig, TexEnvStage},
    stream::{BinReader, BinaryStream},
    system::gpu::{
        colors::Rgba,
        command::{
            ArrayDescriptor, AttributeMode, VertexAttributeStream,
            attributes::{self, Attribute, AttributeDescriptor},
        },
    },
};
use bitos::{
    BitUtils, bitos,
    integer::{UnsignedInt, u3, u4},
};
use gekko::Address;
use glam::{Mat3, Mat4, Vec2, Vec3};
use seq_macro::seq;
use strum::FromRepr;
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

    IndMatxA0 = 0x06,
    IndMatxB0 = 0x07,
    IndMatxC0 = 0x08,
    IndMatxA1 = 0x09,
    IndMatxB1 = 0x0A,
    IndMatxC1 = 0x0B,
    IndMatxA2 = 0x0C,
    IndMatxB2 = 0x0D,
    IndMatxC2 = 0x0E,

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
    RasterIRef = 0x27,

    TevRefs01 = 0x28,
    TevRefs23 = 0x29,
    TevRefs45 = 0x2A,
    TevRefs67 = 0x2B,
    TevRefs89 = 0x2C,
    TevRefsAB = 0x2D,
    TevRefsCD = 0x2E,
    TevRefsEF = 0x2F,

    SetupScaleS0 = 0x30,
    SetupScaleT0 = 0x31,
    SetupScaleS1 = 0x32,
    SetupScaleT1 = 0x33,
    SetupScaleS2 = 0x34,
    SetupScaleT2 = 0x35,
    SetupScaleS3 = 0x36,
    SetupScaleT3 = 0x37,
    SetupScaleS4 = 0x38,
    SetupScaleT4 = 0x39,
    SetupScaleS5 = 0x3A,
    SetupScaleT5 = 0x3B,
    SetupScaleS6 = 0x3C,
    SetupScaleT6 = 0x3D,
    SetupScaleS7 = 0x3E,
    SetupScaleT7 = 0x3F,

    // Pixel Engine
    PixelZMode = 0x40,
    PixelBlendMode = 0x41,
    PixelConstantAlpha = 0x42,
    PixelControl = 0x43,
    PixelFieldMask = 0x44,
    PixelDone = 0x45,
    PixelRefresh = 0x46,
    PixelToken = 0x47,
    PixelTokenInt = 0x48,
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
    TexLoadBlock0 = 0x60,
    TexLoadBlock1 = 0x61,
    TexLoadBlock2 = 0x62,
    TexLoadBlock3 = 0x63,
    TexLoadLut0 = 0x64,
    TexLoadLut1 = 0x65,
    TexInvTags = 0x66,
    TexPerfMode = 0x67,
    TexFieldMode = 0x68,
    TexRefresh = 0x69,

    TexMode0 = 0x80,
    TexMode1 = 0x81,
    TexMode2 = 0x82,
    TexMode3 = 0x83,
    TexMode0Lod = 0x84,
    TexMode1Lod = 0x85,
    TexMode2Lod = 0x86,
    TexMode3Lod = 0x87,
    TexFormat0 = 0x88,
    TexFormat1 = 0x89,
    TexFormat2 = 0x8A,
    TexFormat3 = 0x8B,
    TexEvenLodAddress0 = 0x8C,
    TexEvenLodAddress1 = 0x8D,
    TexEvenLodAddress2 = 0x8E,
    TexEvenLodAddress3 = 0x8F,
    TexOddLodAddress0 = 0x90,
    TexOddLodAddress1 = 0x91,
    TexOddLodAddress2 = 0x92,
    TexOddLodAddress3 = 0x93,
    TexAddress0 = 0x94,
    TexAddress1 = 0x95,
    TexAddress2 = 0x96,
    TexAddress3 = 0x97,

    TexSetLut0 = 0x98,
    TexSetLut1 = 0x99,
    TexSetLut2 = 0x9A,
    TexSetLut3 = 0x9B,

    TexMode4 = 0xA0,
    TexMode5 = 0xA1,
    TexMode6 = 0xA2,
    TexMode7 = 0xA3,
    TexMode4Lod = 0xA4,
    TexMode5Lod = 0xA5,
    TexMode6Lod = 0xA6,
    TexMode7Lod = 0xA7,
    TexFormat4 = 0xA8,
    TexFormat5 = 0xA9,
    TexFormat6 = 0xAA,
    TexFormat7 = 0xAB,
    TexEvenLodAddress4 = 0xAC,
    TexEvenLodAddress5 = 0xAD,
    TexEvenLodAddress6 = 0xAE,
    TexEvenLodAddress7 = 0xAF,
    TexOddLodAddress4 = 0xB0,
    TexOddLodAddress5 = 0xB1,
    TexOddLodAddress6 = 0xB2,
    TexOddLodAddress7 = 0xB3,
    TexAddress4 = 0xB4,
    TexAddress5 = 0xB5,
    TexAddress6 = 0xB6,
    TexAddress7 = 0xB7,

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

    TevConstant3AR = 0xE0,
    TevConstant3GB = 0xE1,
    TevConstant0AR = 0xE2,
    TevConstant0GB = 0xE3,
    TevConstant1AR = 0xE4,
    TevConstant1GB = 0xE5,
    TevConstant2AR = 0xE6,
    TevConstant2GB = 0xE7,

    TevRangeAdjC = 0xE8,
    TevRangeAdj0 = 0xE9,
    TevRangeAdj1 = 0xEA,
    TevRangeAdj2 = 0xEB,
    TevRangeAdj3 = 0xEC,
    TevRangeAdj4 = 0xED,

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
                | Self::TevConstant3AR
                | Self::TevConstant3GB
                | Self::TevConstant0AR
                | Self::TevConstant0GB
                | Self::TevConstant1AR
                | Self::TevConstant1GB
                | Self::TevConstant2AR
                | Self::TevConstant2GB
                | Self::TevKSel0
                | Self::TevKSel1
                | Self::TevKSel2
                | Self::TevKSel3
                | Self::TevKSel4
                | Self::TevKSel5
                | Self::TevKSel6
                | Self::TevKSel7
        )
    }

    pub fn is_pixel_clear(&self) -> bool {
        matches!(
            self,
            Self::PixelCopyClearAr | Self::PixelCopyClearGb | Self::PixelCopyClearZ
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topology {
    QuadList,
    TriangleList,
    TriangleStrip,
    TriangleFan,
    LineList,
    LineStrip,
    PointList,
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
#[derive(Debug, Default)]
pub struct VertexAttributes {
    pub position: Vec3,
    pub position_matrix: Mat4,

    pub normal: Vec3,
    pub normal_matrix: Mat3,

    pub diffuse: Rgba,
    pub specular: Rgba,

    pub tex_coords: [Vec2; 8],
    pub tex_coords_matrix: [Mat4; 8],
}

/// GX subsystem
#[derive(Debug, Default)]
pub struct Gpu {
    pub command: command::Interface,
    pub transform: transform::Interface,
    pub environment: environment::Interface,
    pub texture: texture::Interface,
    pub pixel: pixel::Interface,
}

impl System {
    fn gx_set(&mut self, reg: Reg, value: u32) {
        match reg {
            Reg::GenMode => {
                let mode = GenMode::from_bits(value);
                self.gpu.environment.active_stages = mode.tev_stages_minus_one().value() + 1;
                self.gpu.environment.active_channels = mode.color_channels_count().value();
                tracing::debug!(?mode);
            }

            Reg::TevRefs01 => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[0].as_mut_bytes())
            }
            Reg::TevRefs23 => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[1].as_mut_bytes())
            }
            Reg::TevRefs45 => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[2].as_mut_bytes())
            }
            Reg::TevRefs67 => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[3].as_mut_bytes())
            }
            Reg::TevRefs89 => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[4].as_mut_bytes())
            }
            Reg::TevRefsAB => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[5].as_mut_bytes())
            }
            Reg::TevRefsCD => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[6].as_mut_bytes())
            }
            Reg::TevRefsEF => {
                value.write_ne_bytes(self.gpu.environment.stage_refs[7].as_mut_bytes())
            }

            Reg::SetupScaleS0 => {
                value.write_ne_bytes(self.gpu.texture.maps[0].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT0 => {
                value.write_ne_bytes(self.gpu.texture.maps[0].scaling.t.as_mut_bytes())
            }
            Reg::SetupScaleS1 => {
                value.write_ne_bytes(self.gpu.texture.maps[1].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT1 => {
                value.write_ne_bytes(self.gpu.texture.maps[1].scaling.t.as_mut_bytes())
            }
            Reg::SetupScaleS2 => {
                value.write_ne_bytes(self.gpu.texture.maps[2].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT2 => {
                value.write_ne_bytes(self.gpu.texture.maps[2].scaling.t.as_mut_bytes())
            }
            Reg::SetupScaleS3 => {
                value.write_ne_bytes(self.gpu.texture.maps[3].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT3 => {
                value.write_ne_bytes(self.gpu.texture.maps[3].scaling.t.as_mut_bytes())
            }
            Reg::SetupScaleS4 => {
                value.write_ne_bytes(self.gpu.texture.maps[4].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT4 => {
                value.write_ne_bytes(self.gpu.texture.maps[4].scaling.t.as_mut_bytes())
            }
            Reg::SetupScaleS5 => {
                value.write_ne_bytes(self.gpu.texture.maps[5].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT5 => {
                value.write_ne_bytes(self.gpu.texture.maps[5].scaling.t.as_mut_bytes())
            }
            Reg::SetupScaleS6 => {
                value.write_ne_bytes(self.gpu.texture.maps[6].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT6 => {
                value.write_ne_bytes(self.gpu.texture.maps[6].scaling.t.as_mut_bytes())
            }
            Reg::SetupScaleS7 => {
                value.write_ne_bytes(self.gpu.texture.maps[7].scaling.s.as_mut_bytes())
            }
            Reg::SetupScaleT7 => {
                value.write_ne_bytes(self.gpu.texture.maps[7].scaling.t.as_mut_bytes())
            }

            Reg::PixelZMode => {
                value.write_ne_bytes(self.gpu.pixel.depth_mode.as_mut_bytes());
                self.config
                    .renderer
                    .exec(Action::SetDepthMode(self.gpu.pixel.depth_mode));
            }
            Reg::PixelBlendMode => {
                value.write_ne_bytes(self.gpu.pixel.blend_mode.as_mut_bytes());
                self.config
                    .renderer
                    .exec(Action::SetBlendMode(self.gpu.pixel.blend_mode));
            }
            Reg::PixelConstantAlpha => {
                self.gpu.pixel.constant_alpha = pixel::ConstantAlpha::from_bits(value);
                self.config
                    .renderer
                    .exec(Action::SetConstantAlpha(self.gpu.pixel.constant_alpha));
            }
            Reg::PixelControl => {
                self.gpu.pixel.control = pixel::Control::from_bits(value);
                self.config.renderer.exec(Action::SetFramebufferFormat(
                    self.gpu.pixel.control.format(),
                ));
            }
            Reg::PixelDone => {
                self.gpu.pixel.interrupt.set_finish(true);
                self.scheduler.schedule_now(System::pi_check_interrupts);
            }
            Reg::PixelToken => {
                self.gpu.pixel.token = value;
            }
            Reg::PixelTokenInt => {
                self.gpu.pixel.interrupt.set_token(true);
                self.scheduler.schedule_now(System::pi_check_interrupts);
            }
            Reg::PixelCopyClearAr => {
                self.gpu.pixel.clear_color.r = value.bits(0, 8) as u8;
                self.gpu.pixel.clear_color.a = value.bits(8, 16) as u8;
            }
            Reg::PixelCopyClearGb => {
                self.gpu.pixel.clear_color.b = value.bits(0, 8) as u8;
                self.gpu.pixel.clear_color.g = value.bits(8, 16) as u8;
            }
            Reg::PixelCopyClearZ => value.write_be_bytes(self.gpu.pixel.clear_depth.as_mut_bytes()),
            Reg::PixelCopyCmd => {
                let cmd = pixel::CopyCmd::from_bits(value);
                self.config.renderer.exec(Action::EfbCopy {
                    clear: cmd.clear(),
                    to_xfb: cmd.to_xfb(),
                });
            }

            Reg::TexMode0 => {
                value.write_ne_bytes(self.gpu.texture.maps[0].mode.as_mut_bytes());
                self.gpu.texture.maps[0].dirty = true;
            }
            Reg::TexMode1 => {
                value.write_ne_bytes(self.gpu.texture.maps[1].mode.as_mut_bytes());
                self.gpu.texture.maps[1].dirty = true;
            }
            Reg::TexMode2 => {
                value.write_ne_bytes(self.gpu.texture.maps[2].mode.as_mut_bytes());
                self.gpu.texture.maps[2].dirty = true;
            }
            Reg::TexMode3 => {
                value.write_ne_bytes(self.gpu.texture.maps[3].mode.as_mut_bytes());
                self.gpu.texture.maps[3].dirty = true;
            }
            Reg::TexMode4 => {
                value.write_ne_bytes(self.gpu.texture.maps[4].mode.as_mut_bytes());
                self.gpu.texture.maps[4].dirty = true;
            }
            Reg::TexMode5 => {
                value.write_ne_bytes(self.gpu.texture.maps[5].mode.as_mut_bytes());
                self.gpu.texture.maps[5].dirty = true;
            }
            Reg::TexMode6 => {
                value.write_ne_bytes(self.gpu.texture.maps[6].mode.as_mut_bytes());
                self.gpu.texture.maps[6].dirty = true;
            }
            Reg::TexMode7 => {
                value.write_ne_bytes(self.gpu.texture.maps[7].mode.as_mut_bytes());
                self.gpu.texture.maps[7].dirty = true;
            }

            Reg::TexFormat0 => {
                value.write_ne_bytes(self.gpu.texture.maps[0].format.as_mut_bytes());
                self.gpu.texture.maps[0].dirty = true;
            }
            Reg::TexFormat1 => {
                value.write_ne_bytes(self.gpu.texture.maps[1].format.as_mut_bytes());
                self.gpu.texture.maps[1].dirty = true;
            }
            Reg::TexFormat2 => {
                value.write_ne_bytes(self.gpu.texture.maps[2].format.as_mut_bytes());
                self.gpu.texture.maps[2].dirty = true;
            }
            Reg::TexFormat3 => {
                value.write_ne_bytes(self.gpu.texture.maps[3].format.as_mut_bytes());
                self.gpu.texture.maps[3].dirty = true;
            }
            Reg::TexFormat4 => {
                value.write_ne_bytes(self.gpu.texture.maps[4].format.as_mut_bytes());
                self.gpu.texture.maps[4].dirty = true;
            }
            Reg::TexFormat5 => {
                value.write_ne_bytes(self.gpu.texture.maps[5].format.as_mut_bytes());
                self.gpu.texture.maps[5].dirty = true;
            }
            Reg::TexFormat6 => {
                value.write_ne_bytes(self.gpu.texture.maps[6].format.as_mut_bytes());
                self.gpu.texture.maps[6].dirty = true;
            }
            Reg::TexFormat7 => {
                value.write_ne_bytes(self.gpu.texture.maps[7].format.as_mut_bytes());
                self.gpu.texture.maps[7].dirty = true;
            }

            Reg::TexAddress0 => {
                self.gpu.texture.maps[0].address = Address(value << 5);
                self.gpu.texture.maps[0].dirty = true;
            }
            Reg::TexAddress1 => {
                self.gpu.texture.maps[1].address = Address(value << 5);
                self.gpu.texture.maps[1].dirty = true;
            }
            Reg::TexAddress2 => {
                self.gpu.texture.maps[2].address = Address(value << 5);
                self.gpu.texture.maps[2].dirty = true;
            }
            Reg::TexAddress3 => {
                self.gpu.texture.maps[3].address = Address(value << 5);
                self.gpu.texture.maps[3].dirty = true;
            }
            Reg::TexAddress4 => {
                self.gpu.texture.maps[4].address = Address(value << 5);
                self.gpu.texture.maps[4].dirty = true;
            }
            Reg::TexAddress5 => {
                self.gpu.texture.maps[5].address = Address(value << 5);
                self.gpu.texture.maps[5].dirty = true;
            }
            Reg::TexAddress6 => {
                self.gpu.texture.maps[6].address = Address(value << 5);
                self.gpu.texture.maps[6].dirty = true;
            }
            Reg::TexAddress7 => {
                self.gpu.texture.maps[7].address = Address(value << 5);
                self.gpu.texture.maps[7].dirty = true;
            }

            Reg::TevColor0 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[0].color.as_mut_bytes());
            }
            Reg::TevAlpha0 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[0].alpha.as_mut_bytes());
            }
            Reg::TevColor1 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[1].color.as_mut_bytes());
            }
            Reg::TevAlpha1 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[1].alpha.as_mut_bytes());
            }
            Reg::TevColor2 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[2].color.as_mut_bytes());
            }
            Reg::TevAlpha2 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[2].alpha.as_mut_bytes());
            }
            Reg::TevColor3 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[3].color.as_mut_bytes());
            }
            Reg::TevAlpha3 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[3].alpha.as_mut_bytes());
            }
            Reg::TevColor4 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[4].color.as_mut_bytes());
            }
            Reg::TevAlpha4 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[4].alpha.as_mut_bytes());
            }
            Reg::TevColor5 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[5].color.as_mut_bytes());
            }
            Reg::TevAlpha5 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[5].alpha.as_mut_bytes());
            }
            Reg::TevColor6 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[6].color.as_mut_bytes());
            }
            Reg::TevAlpha6 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[6].alpha.as_mut_bytes());
            }
            Reg::TevColor7 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[7].color.as_mut_bytes());
            }
            Reg::TevAlpha7 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[7].alpha.as_mut_bytes());
            }
            Reg::TevColor8 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[8].color.as_mut_bytes());
            }
            Reg::TevAlpha8 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[8].alpha.as_mut_bytes());
            }
            Reg::TevColor9 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[9].color.as_mut_bytes());
            }
            Reg::TevAlpha9 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[9].alpha.as_mut_bytes());
            }
            Reg::TevColor10 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[10].color.as_mut_bytes());
            }
            Reg::TevAlpha10 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[10].alpha.as_mut_bytes());
            }
            Reg::TevColor11 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[11].color.as_mut_bytes());
            }
            Reg::TevAlpha11 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[11].alpha.as_mut_bytes());
            }
            Reg::TevColor12 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[12].color.as_mut_bytes());
            }
            Reg::TevAlpha12 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[12].alpha.as_mut_bytes());
            }
            Reg::TevColor13 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[13].color.as_mut_bytes());
            }
            Reg::TevAlpha13 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[13].alpha.as_mut_bytes());
            }
            Reg::TevColor14 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[14].color.as_mut_bytes());
            }
            Reg::TevAlpha14 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[14].alpha.as_mut_bytes());
            }
            Reg::TevColor15 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[15].color.as_mut_bytes());
            }
            Reg::TevAlpha15 => {
                value.write_ne_bytes(self.gpu.environment.stage_ops[15].alpha.as_mut_bytes());
            }
            Reg::TevConstant3AR => {
                let r = ((value.bits(0, 11) as i16) << 5) >> 5;
                let a = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[3].a = a as f32 / 255.0;
                self.gpu.environment.constants[3].r = r as f32 / 255.0;
            }
            Reg::TevConstant3GB => {
                let b = ((value.bits(0, 11) as i16) << 5) >> 5;
                let g = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[3].b = b as f32 / 255.0;
                self.gpu.environment.constants[3].g = g as f32 / 255.0;
            }
            Reg::TevConstant0AR => {
                let r = ((value.bits(0, 11) as i16) << 5) >> 5;
                let a = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[0].a = a as f32 / 255.0;
                self.gpu.environment.constants[0].r = r as f32 / 255.0;
            }
            Reg::TevConstant0GB => {
                let b = ((value.bits(0, 11) as i16) << 5) >> 5;
                let g = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[0].b = b as f32 / 255.0;
                self.gpu.environment.constants[0].g = g as f32 / 255.0;
            }
            Reg::TevConstant1AR => {
                let r = ((value.bits(0, 11) as i16) << 5) >> 5;
                let a = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[1].a = a as f32 / 255.0;
                self.gpu.environment.constants[1].r = r as f32 / 255.0;
            }
            Reg::TevConstant1GB => {
                let b = ((value.bits(0, 11) as i16) << 5) >> 5;
                let g = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[1].b = b as f32 / 255.0;
                self.gpu.environment.constants[1].g = g as f32 / 255.0;
            }
            Reg::TevConstant2AR => {
                let r = ((value.bits(0, 11) as i16) << 5) >> 5;
                let a = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[2].a = a as f32 / 255.0;
                self.gpu.environment.constants[2].r = r as f32 / 255.0;
            }
            Reg::TevConstant2GB => {
                let b = ((value.bits(0, 11) as i16) << 5) >> 5;
                let g = ((value.bits(12, 23) as i16) << 5) >> 5;
                self.gpu.environment.constants[2].b = b as f32 / 255.0;
                self.gpu.environment.constants[2].g = g as f32 / 255.0;
            }
            Reg::TevKSel0 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[0].as_mut_bytes());
            }
            Reg::TevKSel1 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[1].as_mut_bytes());
            }
            Reg::TevKSel2 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[2].as_mut_bytes());
            }
            Reg::TevKSel3 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[3].as_mut_bytes());
            }
            Reg::TevKSel4 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[4].as_mut_bytes());
            }
            Reg::TevKSel5 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[5].as_mut_bytes());
            }
            Reg::TevKSel6 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[6].as_mut_bytes());
            }
            Reg::TevKSel7 => {
                value.write_ne_bytes(self.gpu.environment.stage_consts[7].as_mut_bytes());
            }

            _ => tracing::warn!("unimplemented write to internal GX register {reg:?}"),
        }

        if reg.is_tev() {
            let stages = self
                .gpu
                .environment
                .stage_ops
                .iter()
                .take(self.gpu.environment.active_stages as usize)
                .cloned()
                .enumerate()
                .map(|(i, ops)| {
                    let ref_pair = &self.gpu.environment.stage_refs[i / 2];
                    let const_pair = &self.gpu.environment.stage_consts[i / 2];

                    let (refs, color_const, alpha_const) = if i % 2 == 0 {
                        (ref_pair.a(), const_pair.color_a(), const_pair.alpha_a())
                    } else {
                        (ref_pair.b(), const_pair.color_b(), const_pair.alpha_b())
                    };

                    TexEnvStage {
                        ops,
                        refs,
                        color_const,
                        alpha_const,
                    }
                })
                .collect::<Vec<_>>();

            let config = TexEnvConfig {
                stages,
                constants: self.gpu.environment.constants,
            };

            self.config.renderer.exec(Action::SetTexEnvConfig(config));
        }

        if reg.is_pixel_clear() {
            self.config
                .renderer
                .exec(Action::SetClearColor(self.gpu.pixel.clear_color.into()));
        }
    }
}

impl System {
    #[inline(always)]
    fn gx_read_attribute_from_array<D: AttributeDescriptor>(
        &mut self,
        descriptor: &D,
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

    #[inline(always)]
    fn gx_read_attribute<A: Attribute>(
        &mut self,
        vat: usize,
        reader: &mut BinReader,
    ) -> Option<<A::Descriptor as AttributeDescriptor>::Value> {
        let mode = A::get_mode(&self.gpu.command.internal.vertex_descriptor);
        let descriptor = A::get_descriptor(&self.gpu.command.internal.vertex_attr_tables[vat]);

        match mode {
            AttributeMode::None => None,
            AttributeMode::Direct => Some(
                descriptor
                    .read(reader)
                    .unwrap_or_else(|| panic!("failed to read {:?}", A::NAME)),
            ),
            AttributeMode::Index8 => {
                let index = reader.read_be::<u8>().unwrap();
                let array = A::get_array(&self.gpu.command.internal.arrays).unwrap();
                Some(self.gx_read_attribute_from_array(&descriptor, array, index as u16))
            }
            AttributeMode::Index16 => {
                let index = reader.read_be::<u16>().unwrap();
                let array = A::get_array(&self.gpu.command.internal.arrays).unwrap();
                Some(self.gx_read_attribute_from_array(&descriptor, array, index))
            }
        }
    }

    pub fn gx_extract_attributes(
        &mut self,
        stream: &VertexAttributeStream,
    ) -> Vec<VertexAttributes> {
        let vat = stream.table_index();
        let default_pos_matrix_idx = self.gpu.command.internal.mat_indices.view().value();

        let mut vertices = Vec::with_capacity(stream.count() as usize);
        let mut data = stream.data();
        let mut reader = data.reader();
        for _ in 0..stream.count() {
            let position_matrix_index = self
                .gx_read_attribute::<attributes::PosMatrixIndex>(vat, &mut reader)
                .unwrap_or(default_pos_matrix_idx);

            let position_matrix = self.gpu.transform.matrix(position_matrix_index);
            let normal_matrix = self.gpu.transform.normal_matrix(position_matrix_index);

            let mut tex_coords_matrix = [Mat4::ZERO; 8];
            seq! {
                N in 0..8 {
                    let default = self
                        .gpu
                        .command
                        .internal
                        .mat_indices
                        .tex_at(N)
                        .unwrap()
                        .value();

                    let tex_matrix_index = self
                        .gx_read_attribute::<attributes::TexMatrixIndex<N>>(vat, &mut reader)
                        .unwrap_or(default);

                    tex_coords_matrix[N] = self.gpu.transform.matrix(tex_matrix_index);
                }
            }

            let position = self
                .gx_read_attribute::<attributes::Position>(vat, &mut reader)
                .unwrap_or_default();

            let normal = self
                .gx_read_attribute::<attributes::Normal>(vat, &mut reader)
                .unwrap_or_default();

            let diffuse = self
                .gx_read_attribute::<attributes::Diffuse>(vat, &mut reader)
                .unwrap_or_default();

            let specular = self
                .gx_read_attribute::<attributes::Specular>(vat, &mut reader)
                .unwrap_or_default();

            let mut tex_coords = [Vec2::ZERO; 8];
            seq! {
                N in 0..8 {
                    tex_coords[N] = self
                        .gx_read_attribute::<attributes::TexCoords<N>>(vat, &mut reader)
                        .unwrap_or_default();
                }
            }

            vertices.push(VertexAttributes {
                position,
                position_matrix,
                normal,
                normal_matrix,
                diffuse,
                specular,
                tex_coords,
                tex_coords_matrix,
            })
        }

        vertices
    }

    pub fn gx_update_texture(&mut self, index: usize) {
        let map = self.gpu.texture.maps[index];
        let start = map.address.value() as usize;
        let len = map.format.size().value() as usize;
        let slice = &self.mem.ram[start..][..len];

        if !self.gpu.texture.insert_cache(map.address, slice) {
            let data = texture::decode_texture(slice, map.format);
            self.config.renderer.exec(Action::LoadTexture {
                id: map.address.value(),
                width: map.format.width(),
                height: map.format.height(),
                data,
            });
        }

        self.config.renderer.exec(Action::SetTexture {
            index,
            id: map.address.value(),
        });
    }

    pub fn gx_call(&mut self, address: Address, length: u32) {
        tracing::debug!("called {} with length 0x{:08X}", address, length);
        let address = address.value().with_bits(26, 32, 0);
        let data = &self.mem.ram[address.value() as usize..][..length as usize];
        self.gpu.command.queue.push_front_bytes(data);
    }

    pub fn gx_draw(&mut self, topology: Topology, stream: &VertexAttributeStream) {
        if std::mem::take(&mut self.gpu.transform.internal.viewport_dirty) {
            self.config
                .renderer
                .exec(Action::SetViewport(crate::render::Viewport {
                    width: self.gpu.transform.internal.viewport.width,
                    height: self.gpu.transform.internal.viewport.height,
                    top_left_x: self.gpu.transform.internal.viewport.center_x
                        - self.gpu.transform.internal.viewport.width / 2.0,
                    top_left_y: self.gpu.transform.internal.viewport.center_y
                        - self.gpu.transform.internal.viewport.height / 2.0,
                    far_z: self.gpu.transform.internal.viewport.far,
                    near_z: self.gpu.transform.internal.viewport.far
                        - self.gpu.transform.internal.viewport.far_minus_near,
                }));
        }

        for map in 0..8 {
            if !self.gpu.texture.maps[map].dirty {
                continue;
            }

            self.gpu.texture.maps[map].dirty = false;
            self.gx_update_texture(map);
        }

        let attributes = self.gx_extract_attributes(stream);
        self.config
            .renderer
            .exec(Action::Draw(topology, attributes));
    }
}
