//! Graphics subsystem (GX).
pub mod colors;

pub mod cmd;
pub mod pix;
pub mod tev;
pub mod tex;
pub mod xf;

use crate::{
    Primitive, System,
    modules::render,
    stream::{BinReader, BinaryStream},
    system::{
        gx::{
            cmd::{
                ArrayDescriptor, AttributeMode, VertexAttributeStream,
                attributes::{self, Attribute, AttributeDescriptor},
            },
            colors::Rgba,
            tex::{encode_color_texture, encode_depth_texture},
        },
        pi,
    },
};
use bitos::{
    BitUtils, TryBits, bitos,
    integer::{UnsignedInt, u3, u4},
};
use gekko::Address;
use glam::{Mat4, Vec2, Vec3};
use ring_arena::{Handle, RingArena};
use seq_macro::seq;
use std::{
    num::NonZero,
    sync::{LazyLock, Mutex},
};
use strum::FromRepr;
use tinyvec::TinyVec;
use zerocopy::IntoBytes;

/// Maximum value for the 24-bit depth.
pub const DEPTH_24_BIT_MAX: u32 = (1 << 24) - 1;

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
    PixelCopySrc = 0x49,        // texCopyTL
    PixelCopyDimensions = 0x4A, // texCopyHW
    PixelCopyDst = 0x4B,        // not a register in libogc, set manually
    PixelCopyDstStride = 0x4D,  // texCopyDst
    PixelCopyScale = 0x4E,
    PixelCopyClearAr = 0x4F,
    PixelCopyClearGb = 0x50,
    PixelCopyClearZ = 0x51,
    PixelCopyCmd = 0x52, // texCopyCtrl
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
    TexLutAddress = 0x64,
    TexLutCount = 0x65,
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
    TexLutRef0 = 0x98,
    TexLutRef1 = 0x99,
    TexLutRef2 = 0x9A,
    TexLutRef3 = 0x9B,

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
    TexLutRef4 = 0xB8,
    TexLutRef5 = 0xB9,
    TexLutRef6 = 0xBA,
    TexLutRef7 = 0xBB,

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
    WriteMask = 0xFE,
}

impl Reg {
    pub fn is_tev(&self) -> bool {
        matches!(
            self,
            Self::GenMode
                | Self::TevColor0
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

type MatrixId = u16;

/// A vertex extracted from a [`VertexAttributeStream`].
#[derive(Debug, Default)]
pub struct Vertex {
    pub position: Vec3,
    pub position_matrix: MatrixId,

    pub normal: Vec3,
    pub normal_matrix: MatrixId,

    pub diffuse: Rgba,
    pub specular: Rgba,

    pub tex_coords: [Vec2; 8],
    pub tex_coords_matrix: [MatrixId; 8],
}

/// A stream of [`Vertex`] elements and their associated matrices.
pub struct VertexStream {
    vertices: Handle<Vertex>,
    matrices: Handle<Mat4>,
}

impl VertexStream {
    pub fn vertices(&self) -> &[Vertex] {
        // SAFETY: this struct is only created inside `extract_vertices`, which mantains
        // a static arena
        unsafe { self.vertices.as_slice().assume_init_ref() }
    }

    pub fn matrices(&self) -> &[Mat4] {
        // SAFETY: this struct is only created inside `extract_vertices`, which mantains
        // a static arena
        unsafe { self.matrices.as_slice().assume_init_ref() }
    }
}

struct MatrixMapping {
    index: u8,
    normal: bool,
}

pub struct Gpu {
    pub mode: GenMode,
    pub command: cmd::Interface,
    pub transform: xf::Interface,
    pub environment: tev::Interface,
    pub texture: tex::Interface,
    pub pixel: pix::Interface,
    pub write_mask: u32,
    matrix_map: Vec<MatrixMapping>,
}

impl Default for Gpu {
    fn default() -> Self {
        Self {
            mode: Default::default(),
            command: Default::default(),
            transform: Default::default(),
            environment: Default::default(),
            texture: Default::default(),
            pixel: Default::default(),
            write_mask: 0x00FF_FFFF,
            matrix_map: Vec::with_capacity(32),
        }
    }
}

pub fn update_texenv(sys: &mut System) {
    let stages = sys
        .gpu
        .environment
        .stage_ops
        .iter()
        .take(sys.gpu.environment.active_stages as usize)
        .cloned()
        .enumerate()
        .map(|(i, ops)| {
            let ref_pair = &sys.gpu.environment.stage_refs[i / 2];
            let const_pair = &sys.gpu.environment.stage_consts[i / 2];

            let (refs, color_const, alpha_const) = if i % 2 == 0 {
                (ref_pair.a(), const_pair.color_a(), const_pair.alpha_a())
            } else {
                (ref_pair.b(), const_pair.color_b(), const_pair.alpha_b())
            };

            render::TexEnvStage {
                ops,
                refs,
                color_const,
                alpha_const,
            }
        })
        .collect::<TinyVec<_>>();

    let config = render::TexEnvConfig {
        stages,
        constants: sys.gpu.environment.constants,
    };

    sys.modules
        .render
        .exec(render::Action::SetTexEnvConfig(config));
}

pub fn set_register(sys: &mut System, reg: Reg, value: u32) {
    let mask = std::mem::replace(&mut sys.gpu.write_mask, 0x00FF_FFFF);
    let masked = value & mask;

    macro_rules! write_masked {
        ($value:expr) => {{
            let old = $value.to_bits() & !mask;
            let new = old | masked;
            new.write_ne_bytes($value.as_mut_bytes());
        }};
    }

    match reg {
        Reg::GenMode => {
            write_masked!(sys.gpu.mode);
            let mode = &sys.gpu.mode;
            sys.gpu.environment.active_stages = mode.tev_stages_minus_one().value() + 1;
            sys.gpu.environment.active_channels = mode.color_channels_count().value();
            tracing::debug!(?mode);
        }

        Reg::TevRefs01 => write_masked!(sys.gpu.environment.stage_refs[0]),
        Reg::TevRefs23 => write_masked!(sys.gpu.environment.stage_refs[1]),
        Reg::TevRefs45 => write_masked!(sys.gpu.environment.stage_refs[2]),
        Reg::TevRefs67 => write_masked!(sys.gpu.environment.stage_refs[3]),
        Reg::TevRefs89 => write_masked!(sys.gpu.environment.stage_refs[4]),
        Reg::TevRefsAB => write_masked!(sys.gpu.environment.stage_refs[5]),
        Reg::TevRefsCD => write_masked!(sys.gpu.environment.stage_refs[6]),
        Reg::TevRefsEF => write_masked!(sys.gpu.environment.stage_refs[7]),

        Reg::SetupScaleS0 => write_masked!(sys.gpu.texture.maps[0].scaling.s),
        Reg::SetupScaleT0 => write_masked!(sys.gpu.texture.maps[0].scaling.t),
        Reg::SetupScaleS1 => write_masked!(sys.gpu.texture.maps[1].scaling.s),
        Reg::SetupScaleT1 => write_masked!(sys.gpu.texture.maps[1].scaling.t),
        Reg::SetupScaleS2 => write_masked!(sys.gpu.texture.maps[2].scaling.s),
        Reg::SetupScaleT2 => write_masked!(sys.gpu.texture.maps[2].scaling.t),
        Reg::SetupScaleS3 => write_masked!(sys.gpu.texture.maps[3].scaling.s),
        Reg::SetupScaleT3 => write_masked!(sys.gpu.texture.maps[3].scaling.t),
        Reg::SetupScaleS4 => write_masked!(sys.gpu.texture.maps[4].scaling.s),
        Reg::SetupScaleT4 => write_masked!(sys.gpu.texture.maps[4].scaling.t),
        Reg::SetupScaleS5 => write_masked!(sys.gpu.texture.maps[5].scaling.s),
        Reg::SetupScaleT5 => write_masked!(sys.gpu.texture.maps[5].scaling.t),
        Reg::SetupScaleS6 => write_masked!(sys.gpu.texture.maps[6].scaling.s),
        Reg::SetupScaleT6 => write_masked!(sys.gpu.texture.maps[6].scaling.t),
        Reg::SetupScaleS7 => write_masked!(sys.gpu.texture.maps[7].scaling.s),
        Reg::SetupScaleT7 => write_masked!(sys.gpu.texture.maps[7].scaling.t),

        Reg::PixelZMode => {
            write_masked!(sys.gpu.pixel.depth_mode);
            sys.modules
                .render
                .exec(render::Action::SetDepthMode(sys.gpu.pixel.depth_mode));
        }
        Reg::PixelBlendMode => {
            write_masked!(sys.gpu.pixel.blend_mode);
            sys.modules
                .render
                .exec(render::Action::SetBlendMode(sys.gpu.pixel.blend_mode));
        }
        Reg::PixelConstantAlpha => {
            write_masked!(sys.gpu.pixel.constant_alpha);
            sys.modules.render.exec(render::Action::SetConstantAlpha(
                sys.gpu.pixel.constant_alpha,
            ));
        }
        Reg::PixelControl => {
            write_masked!(sys.gpu.pixel.control);
            sys.modules
                .render
                .exec(render::Action::SetFramebufferFormat(
                    sys.gpu.pixel.control.format(),
                ));
        }
        Reg::PixelDone => {
            sys.gpu.pixel.interrupt.set_finish(true);
            sys.scheduler.schedule_now(pi::check_interrupts);
        }
        Reg::PixelToken => write_masked!(sys.gpu.pixel.token),
        Reg::PixelTokenInt => {
            sys.gpu.pixel.interrupt.set_token(true);
            sys.scheduler.schedule_now(pi::check_interrupts);
        }
        Reg::PixelCopySrc => write_masked!(sys.gpu.pixel.copy_src),
        Reg::PixelCopyDimensions => write_masked!(sys.gpu.pixel.copy_dimensions),
        Reg::PixelCopyDst => {
            let mut value = sys.gpu.pixel.copy_dst.value() >> 5;
            write_masked!(value);
            sys.gpu.pixel.copy_dst = Address(value << 5);
        }
        Reg::PixelCopyDstStride => write_masked!(sys.gpu.pixel.copy_stride),
        Reg::PixelCopyClearAr => {
            let mut value = 0
                .with_bits(0, 8, sys.gpu.pixel.clear_color.r as u32)
                .with_bits(8, 16, sys.gpu.pixel.clear_color.a as u32);
            write_masked!(value);
            sys.gpu.pixel.clear_color.r = value.bits(0, 8) as u8;
            sys.gpu.pixel.clear_color.a = value.bits(8, 16) as u8;
        }
        Reg::PixelCopyClearGb => {
            let mut value = 0
                .with_bits(0, 8, sys.gpu.pixel.clear_color.b as u32)
                .with_bits(8, 16, sys.gpu.pixel.clear_color.g as u32);
            write_masked!(value);
            sys.gpu.pixel.clear_color.b = value.bits(0, 8) as u8;
            sys.gpu.pixel.clear_color.g = value.bits(8, 16) as u8;
        }
        Reg::PixelCopyClearZ => {
            write_masked!(sys.gpu.pixel.clear_depth);
            sys.modules.render.exec(render::Action::SetClearDepth(
                sys.gpu.pixel.clear_depth as f32 / DEPTH_24_BIT_MAX as f32,
            ));
        }
        Reg::PixelCopyCmd => {
            // TODO: proper masked
            let cmd = pix::CopyCmd::from_bits(value);
            do_efb_copy(sys, cmd);
        }

        Reg::TexLutAddress => {
            // println!("lut address: {}", Address(value.bits(0, 21)));
        }
        Reg::TexLutCount => {
            // println!("lut count: {:?}", LutCount::from_bits(value));
        }

        Reg::TexMode0 => {
            write_masked!(sys.gpu.texture.maps[0].mode);
            sys.gpu.texture.maps[0].dirty = true;
        }
        Reg::TexMode1 => {
            write_masked!(sys.gpu.texture.maps[1].mode);
            sys.gpu.texture.maps[1].dirty = true;
        }
        Reg::TexMode2 => {
            write_masked!(sys.gpu.texture.maps[2].mode);
            sys.gpu.texture.maps[2].dirty = true;
        }
        Reg::TexMode3 => {
            write_masked!(sys.gpu.texture.maps[3].mode);
            sys.gpu.texture.maps[3].dirty = true;
        }
        Reg::TexMode4 => {
            write_masked!(sys.gpu.texture.maps[4].mode);
            sys.gpu.texture.maps[4].dirty = true;
        }
        Reg::TexMode5 => {
            write_masked!(sys.gpu.texture.maps[5].mode);
            sys.gpu.texture.maps[5].dirty = true;
        }
        Reg::TexMode6 => {
            write_masked!(sys.gpu.texture.maps[6].mode);
            sys.gpu.texture.maps[6].dirty = true;
        }
        Reg::TexMode7 => {
            write_masked!(sys.gpu.texture.maps[7].mode);
            sys.gpu.texture.maps[7].dirty = true;
        }

        Reg::TexFormat0 => {
            write_masked!(sys.gpu.texture.maps[0].format);
            sys.gpu.texture.maps[0].dirty = true;
        }
        Reg::TexFormat1 => {
            write_masked!(sys.gpu.texture.maps[1].format);
            sys.gpu.texture.maps[1].dirty = true;
        }
        Reg::TexFormat2 => {
            write_masked!(sys.gpu.texture.maps[2].format);
            sys.gpu.texture.maps[2].dirty = true;
        }
        Reg::TexFormat3 => {
            write_masked!(sys.gpu.texture.maps[3].format);
            sys.gpu.texture.maps[3].dirty = true;
        }
        Reg::TexFormat4 => {
            write_masked!(sys.gpu.texture.maps[4].format);
            sys.gpu.texture.maps[4].dirty = true;
        }
        Reg::TexFormat5 => {
            write_masked!(sys.gpu.texture.maps[5].format);
            sys.gpu.texture.maps[5].dirty = true;
        }
        Reg::TexFormat6 => {
            write_masked!(sys.gpu.texture.maps[6].format);
            sys.gpu.texture.maps[6].dirty = true;
        }
        Reg::TexFormat7 => {
            write_masked!(sys.gpu.texture.maps[7].format);
            sys.gpu.texture.maps[7].dirty = true;
        }

        Reg::TexAddress0 => {
            let mut value = sys.gpu.texture.maps[0].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[0].address = Address(value << 5);
            sys.gpu.texture.maps[0].dirty = true;
        }
        Reg::TexAddress1 => {
            let mut value = sys.gpu.texture.maps[1].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[1].address = Address(value << 5);
            sys.gpu.texture.maps[1].dirty = true;
        }
        Reg::TexAddress2 => {
            let mut value = sys.gpu.texture.maps[2].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[2].address = Address(value << 5);
            sys.gpu.texture.maps[2].dirty = true;
        }
        Reg::TexAddress3 => {
            let mut value = sys.gpu.texture.maps[3].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[3].address = Address(value << 5);
            sys.gpu.texture.maps[3].dirty = true;
        }
        Reg::TexAddress4 => {
            let mut value = sys.gpu.texture.maps[4].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[4].address = Address(value << 5);
            sys.gpu.texture.maps[4].dirty = true;
        }
        Reg::TexAddress5 => {
            let mut value = sys.gpu.texture.maps[5].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[5].address = Address(value << 5);
            sys.gpu.texture.maps[5].dirty = true;
        }
        Reg::TexAddress6 => {
            let mut value = sys.gpu.texture.maps[6].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[6].address = Address(value << 5);
            sys.gpu.texture.maps[6].dirty = true;
        }
        Reg::TexAddress7 => {
            let mut value = sys.gpu.texture.maps[7].address.value() >> 5;
            write_masked!(value);
            sys.gpu.texture.maps[7].address = Address(value << 5);
            sys.gpu.texture.maps[7].dirty = true;
        }

        Reg::TexLutRef0 => {
            write_masked!(sys.gpu.texture.maps[0].lut);
            sys.gpu.texture.maps[0].dirty = true;
        }
        Reg::TexLutRef1 => {
            write_masked!(sys.gpu.texture.maps[1].lut);
            sys.gpu.texture.maps[1].dirty = true;
        }
        Reg::TexLutRef2 => {
            write_masked!(sys.gpu.texture.maps[2].lut);
            sys.gpu.texture.maps[2].dirty = true;
        }
        Reg::TexLutRef3 => {
            write_masked!(sys.gpu.texture.maps[3].lut);
            sys.gpu.texture.maps[3].dirty = true;
        }
        Reg::TexLutRef4 => {
            write_masked!(sys.gpu.texture.maps[4].lut);
            sys.gpu.texture.maps[4].dirty = true;
        }
        Reg::TexLutRef5 => {
            write_masked!(sys.gpu.texture.maps[5].lut);
            sys.gpu.texture.maps[5].dirty = true;
        }
        Reg::TexLutRef6 => {
            write_masked!(sys.gpu.texture.maps[6].lut);
            sys.gpu.texture.maps[6].dirty = true;
        }
        Reg::TexLutRef7 => {
            write_masked!(sys.gpu.texture.maps[7].lut);
            sys.gpu.texture.maps[7].dirty = true;
        }

        Reg::TevColor0 => {
            write_masked!(sys.gpu.environment.stage_ops[0].color);
        }
        Reg::TevAlpha0 => {
            write_masked!(sys.gpu.environment.stage_ops[0].alpha);
        }
        Reg::TevColor1 => {
            write_masked!(sys.gpu.environment.stage_ops[1].color);
        }
        Reg::TevAlpha1 => {
            write_masked!(sys.gpu.environment.stage_ops[1].alpha);
        }
        Reg::TevColor2 => {
            write_masked!(sys.gpu.environment.stage_ops[2].color);
        }
        Reg::TevAlpha2 => {
            write_masked!(sys.gpu.environment.stage_ops[2].alpha);
        }
        Reg::TevColor3 => {
            write_masked!(sys.gpu.environment.stage_ops[3].color);
        }
        Reg::TevAlpha3 => {
            write_masked!(sys.gpu.environment.stage_ops[3].alpha);
        }
        Reg::TevColor4 => {
            write_masked!(sys.gpu.environment.stage_ops[4].color);
        }
        Reg::TevAlpha4 => {
            write_masked!(sys.gpu.environment.stage_ops[4].alpha);
        }
        Reg::TevColor5 => {
            write_masked!(sys.gpu.environment.stage_ops[5].color);
        }
        Reg::TevAlpha5 => {
            write_masked!(sys.gpu.environment.stage_ops[5].alpha);
        }
        Reg::TevColor6 => {
            write_masked!(sys.gpu.environment.stage_ops[6].color);
        }
        Reg::TevAlpha6 => {
            write_masked!(sys.gpu.environment.stage_ops[6].alpha);
        }
        Reg::TevColor7 => {
            write_masked!(sys.gpu.environment.stage_ops[7].color);
        }
        Reg::TevAlpha7 => {
            write_masked!(sys.gpu.environment.stage_ops[7].alpha);
        }
        Reg::TevColor8 => {
            write_masked!(sys.gpu.environment.stage_ops[8].color);
        }
        Reg::TevAlpha8 => {
            write_masked!(sys.gpu.environment.stage_ops[8].alpha);
        }
        Reg::TevColor9 => {
            write_masked!(sys.gpu.environment.stage_ops[9].color);
        }
        Reg::TevAlpha9 => {
            write_masked!(sys.gpu.environment.stage_ops[9].alpha);
        }
        Reg::TevColor10 => {
            write_masked!(sys.gpu.environment.stage_ops[10].color);
        }
        Reg::TevAlpha10 => {
            write_masked!(sys.gpu.environment.stage_ops[10].alpha);
        }
        Reg::TevColor11 => {
            write_masked!(sys.gpu.environment.stage_ops[11].color);
        }
        Reg::TevAlpha11 => {
            write_masked!(sys.gpu.environment.stage_ops[11].alpha);
        }
        Reg::TevColor12 => {
            write_masked!(sys.gpu.environment.stage_ops[12].color);
        }
        Reg::TevAlpha12 => {
            write_masked!(sys.gpu.environment.stage_ops[12].alpha);
        }
        Reg::TevColor13 => {
            write_masked!(sys.gpu.environment.stage_ops[13].color);
        }
        Reg::TevAlpha13 => {
            write_masked!(sys.gpu.environment.stage_ops[13].alpha);
        }
        Reg::TevColor14 => {
            write_masked!(sys.gpu.environment.stage_ops[14].color);
        }
        Reg::TevAlpha14 => {
            write_masked!(sys.gpu.environment.stage_ops[14].alpha);
        }
        Reg::TevColor15 => {
            write_masked!(sys.gpu.environment.stage_ops[15].color);
        }
        Reg::TevAlpha15 => {
            write_masked!(sys.gpu.environment.stage_ops[15].alpha);
        }
        Reg::TevConstant3AR => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let r = ((value.bits(0, 11) as i16) << 5) >> 5;
            let a = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[3].a = a as f32 / 255.0;
            sys.gpu.environment.constants[3].r = r as f32 / 255.0;
        }
        Reg::TevConstant3GB => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let b = ((value.bits(0, 11) as i16) << 5) >> 5;
            let g = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[3].b = b as f32 / 255.0;
            sys.gpu.environment.constants[3].g = g as f32 / 255.0;
        }
        Reg::TevConstant0AR => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let r = ((value.bits(0, 11) as i16) << 5) >> 5;
            let a = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[0].a = a as f32 / 255.0;
            sys.gpu.environment.constants[0].r = r as f32 / 255.0;
        }
        Reg::TevConstant0GB => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let b = ((value.bits(0, 11) as i16) << 5) >> 5;
            let g = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[0].b = b as f32 / 255.0;
            sys.gpu.environment.constants[0].g = g as f32 / 255.0;
        }
        Reg::TevConstant1AR => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let r = ((value.bits(0, 11) as i16) << 5) >> 5;
            let a = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[1].a = a as f32 / 255.0;
            sys.gpu.environment.constants[1].r = r as f32 / 255.0;
        }
        Reg::TevConstant1GB => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let b = ((value.bits(0, 11) as i16) << 5) >> 5;
            let g = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[1].b = b as f32 / 255.0;
            sys.gpu.environment.constants[1].g = g as f32 / 255.0;
        }
        Reg::TevConstant2AR => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let r = ((value.bits(0, 11) as i16) << 5) >> 5;
            let a = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[2].a = a as f32 / 255.0;
            sys.gpu.environment.constants[2].r = r as f32 / 255.0;
        }
        Reg::TevConstant2GB => {
            if mask != 0x00FF_FFFF {
                todo!();
            }

            let b = ((value.bits(0, 11) as i16) << 5) >> 5;
            let g = ((value.bits(12, 23) as i16) << 5) >> 5;
            sys.gpu.environment.constants[2].b = b as f32 / 255.0;
            sys.gpu.environment.constants[2].g = g as f32 / 255.0;
        }
        Reg::TevAlphaFunc => {
            write_masked!(sys.gpu.environment.alpha_function);
            sys.modules.render.exec(render::Action::SetAlphaFunction(
                sys.gpu.environment.alpha_function.clone(),
            ));
        }
        Reg::TevKSel0 => {
            write_masked!(sys.gpu.environment.stage_consts[0]);
        }
        Reg::TevKSel1 => {
            write_masked!(sys.gpu.environment.stage_consts[1]);
        }
        Reg::TevKSel2 => {
            write_masked!(sys.gpu.environment.stage_consts[2]);
        }
        Reg::TevKSel3 => {
            write_masked!(sys.gpu.environment.stage_consts[3]);
        }
        Reg::TevKSel4 => {
            write_masked!(sys.gpu.environment.stage_consts[4]);
        }
        Reg::TevKSel5 => {
            write_masked!(sys.gpu.environment.stage_consts[5]);
        }
        Reg::TevKSel6 => {
            write_masked!(sys.gpu.environment.stage_consts[6]);
        }
        Reg::TevKSel7 => {
            write_masked!(sys.gpu.environment.stage_consts[7]);
        }
        Reg::WriteMask => {
            sys.gpu.write_mask = value;
        }
        _ => {
            tracing::warn!("unimplemented write to internal GX register {reg:?}: 0x{value:06X}")
        }
    }

    if reg == Reg::GenMode {
        xf::update_texgen(sys);
    }

    if reg.is_tev() {
        sys.gpu.environment.stages_dirty = true;
    }

    if reg.is_pixel_clear() {
        sys.modules.render.exec(render::Action::SetClearColor(
            sys.gpu.pixel.clear_color.into(),
        ));
    }
}

#[inline(always)]
fn read_attribute_from_array<D: AttributeDescriptor>(
    sys: &mut System,
    descriptor: &D,
    array: ArrayDescriptor,
    index: u16,
) -> D::Value {
    let base = array.address.value() as usize;
    let offset = array.stride.value() as usize * index as usize;
    let address = base + offset;
    let mut array = &sys.mem.ram[address..];
    let mut reader = array.reader();
    descriptor.read(&mut reader).unwrap()
}

#[inline(always)]
fn read_attribute<A: Attribute>(
    sys: &mut System,
    vat: usize,
    reader: &mut BinReader,
) -> Option<<A::Descriptor as AttributeDescriptor>::Value> {
    let mode = A::get_mode(&sys.gpu.command.internal.vertex_descriptor);
    let descriptor = A::get_descriptor(&sys.gpu.command.internal.vertex_attr_tables[vat]);

    match mode {
        AttributeMode::None => None,
        AttributeMode::Direct => Some(
            descriptor
                .read(reader)
                .unwrap_or_else(|| panic!("failed to read {:?}", A::NAME)),
        ),
        AttributeMode::Index8 => {
            let index = reader.read_be::<u8>().unwrap() as u16;
            let array = A::get_array(&sys.gpu.command.internal.arrays).unwrap();
            Some(read_attribute_from_array(sys, &descriptor, array, index))
        }
        AttributeMode::Index16 => {
            let index = reader.read_be::<u16>().unwrap();
            let array = A::get_array(&sys.gpu.command.internal.arrays).unwrap();
            Some(read_attribute_from_array(sys, &descriptor, array, index))
        }
    }
}

#[inline]
fn alloc_vertices_handle(length: usize) -> Handle<Vertex> {
    const CHUNK_SIZE: usize = 1 * bytesize::MIB as usize;
    const CHUNK_CAPACITY: NonZero<usize> = NonZero::new(CHUNK_SIZE / size_of::<Vertex>()).unwrap();

    static ARENA: LazyLock<Mutex<RingArena<Vertex>>> =
        LazyLock::new(|| Mutex::new(RingArena::new(CHUNK_CAPACITY)));

    ARENA.lock().unwrap().allocate(length)
}

#[inline]
fn alloc_matrices_handle(length: usize) -> Handle<Mat4> {
    const CHUNK_SIZE: usize = 2 * bytesize::MIB as usize;
    const CHUNK_CAPACITY: NonZero<usize> = NonZero::new(CHUNK_SIZE / size_of::<Mat4>()).unwrap();

    static ARENA: LazyLock<Mutex<RingArena<Mat4>>> =
        LazyLock::new(|| Mutex::new(RingArena::new(CHUNK_CAPACITY)));

    ARENA.lock().unwrap().allocate(length)
}

fn get_matrix_id(sys: &mut System, index: u8, normal: bool) -> MatrixId {
    let id = sys
        .gpu
        .matrix_map
        .iter()
        .position(|m| m.normal == normal && m.index == index);

    if let Some(id) = id {
        id as MatrixId
    } else {
        sys.gpu.matrix_map.push(MatrixMapping { index, normal });
        sys.gpu.matrix_map.len() as MatrixId - 1
    }
}

fn extract_vertices(sys: &mut System, stream: &VertexAttributeStream) -> VertexStream {
    let vat = stream.table_index();
    let default_pos_matrix_idx = sys.gpu.transform.internal.mat_indices.view().value();

    sys.gpu.matrix_map.clear();

    let mut vertices = alloc_vertices_handle(stream.count() as usize);
    let vertices_slice = unsafe { vertices.as_mut_slice() };

    let mut data = stream.data();
    let mut reader = data.reader();
    for i in 0..stream.count() {
        let position_matrix_index =
            read_attribute::<attributes::PosMatrixIndex>(sys, vat, &mut reader)
                .unwrap_or(default_pos_matrix_idx);

        let position_matrix = self::get_matrix_id(sys, position_matrix_index, false);
        let normal_matrix = self::get_matrix_id(sys, position_matrix_index, true);

        let mut tex_coords_matrix = [0; 8];
        seq! {
            N in 0..8 {
                let default = sys
                    .gpu
                    .transform
                    .internal
                    .mat_indices
                    .tex_at(N)
                    .unwrap()
                    .value();

                let tex_matrix_index =
                    read_attribute::<attributes::TexMatrixIndex<N>>(sys, vat, &mut reader)
                    .unwrap_or(default);

                tex_coords_matrix[N] = self::get_matrix_id(sys, tex_matrix_index, false);
            }
        }

        let position =
            read_attribute::<attributes::Position>(sys, vat, &mut reader).unwrap_or_default();

        let normal =
            read_attribute::<attributes::Normal>(sys, vat, &mut reader).unwrap_or_default();

        let diffuse =
            read_attribute::<attributes::Diffuse>(sys, vat, &mut reader).unwrap_or_default();

        let specular =
            read_attribute::<attributes::Specular>(sys, vat, &mut reader).unwrap_or_default();

        let mut tex_coords = [Vec2::ZERO; 8];
        seq! {
            N in 0..8 {
                tex_coords[N] =
                    read_attribute::<attributes::TexCoords<N>>(sys, vat, &mut reader)
                    .unwrap_or_default();
            }
        }

        vertices_slice[i as usize].write(Vertex {
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

    let mut matrices = alloc_matrices_handle(sys.gpu.matrix_map.len());
    let matrices_slice = unsafe { matrices.as_mut_slice() };

    for (id, mapping) in sys.gpu.matrix_map.iter().enumerate() {
        let mat = if mapping.normal {
            Mat4::from_mat3(sys.gpu.transform.normal_matrix(mapping.index))
        } else {
            sys.gpu.transform.matrix(mapping.index)
        };

        matrices_slice[id as usize].write(mat);
    }

    VertexStream { vertices, matrices }
}

fn update_texture(sys: &mut System, index: usize) {
    let map = sys.gpu.texture.maps[index];
    let start = map.address.value() as usize;
    let len = map.format.size().value() as usize;
    let slice = &sys.mem.ram[start..][..len];

    if !sys.gpu.texture.insert_cache(map.address, slice) {
        // println!("READING TEXTURE FROM {}", map.address);
        let data = tex::decode_texture(slice, map.format);
        sys.modules.render.exec(render::Action::LoadTexture {
            id: map.address.value(),
            width: map.format.width(),
            height: map.format.height(),
            data,
        });
    }

    sys.modules.render.exec(render::Action::SetTexture {
        index,
        id: map.address.value(),
    });
}

fn call(sys: &mut System, address: Address, length: u32) {
    tracing::debug!("called {} with length 0x{:08X}", address, length);
    let address = address.value().with_bits(26, 32, 0);
    let data = &sys.mem.ram[address.value() as usize..][..length as usize];
    sys.gpu.command.queue.push_front_bytes(data);
}

fn draw(sys: &mut System, topology: Topology, stream: &VertexAttributeStream) {
    if std::mem::take(&mut sys.gpu.transform.internal.viewport_dirty) {
        sys.modules
            .render
            .exec(render::Action::SetViewport(render::Viewport {
                width: sys.gpu.transform.internal.viewport.width,
                height: sys.gpu.transform.internal.viewport.height,
                top_left_x: sys.gpu.transform.internal.viewport.center_x
                    - sys.gpu.transform.internal.viewport.width / 2.0,
                top_left_y: sys.gpu.transform.internal.viewport.center_y
                    - sys.gpu.transform.internal.viewport.height / 2.0,
                far_z: sys.gpu.transform.internal.viewport.far,
                near_z: sys.gpu.transform.internal.viewport.far
                    - sys.gpu.transform.internal.viewport.far_minus_near,
            }));
    }

    if std::mem::take(&mut sys.gpu.transform.internal.stages_dirty) {
        xf::update_texgen(sys);
    }

    if std::mem::take(&mut sys.gpu.environment.stages_dirty) {
        self::update_texenv(sys);
    }

    for map in 0..8 {
        if std::mem::take(&mut sys.gpu.texture.maps[map].dirty) {
            self::update_texture(sys, map);
        }
    }

    let attributes = self::extract_vertices(sys, stream);
    sys.modules
        .render
        .exec(render::Action::Draw(topology, attributes));
}

fn do_efb_copy(sys: &mut System, cmd: pix::CopyCmd) {
    if cmd.to_xfb() {
        sys.modules
            .render
            .exec(render::Action::XfbCopy { clear: cmd.clear() });
        return;
    }

    if sys.gpu.pixel.control.format().is_depth() {
        let (sender, receiver) = oneshot::channel();
        let width = sys.gpu.pixel.copy_dimensions.width();
        let height = sys.gpu.pixel.copy_dimensions.height();
        sys.modules.render.exec(render::Action::DepthCopy {
            x: sys.gpu.pixel.copy_src.x().value(),
            y: sys.gpu.pixel.copy_src.y().value(),
            width,
            height,
            half: cmd.half(),
            clear: cmd.clear(),
            response: sender,
        });

        let divisor = if cmd.half() { 2 } else { 1 };
        let pixels = receiver.recv().unwrap();
        let stride = sys.gpu.pixel.copy_stride;
        let width = sys.gpu.pixel.copy_dimensions.width() as u32 / divisor;
        let height = sys.gpu.pixel.copy_dimensions.height() as u32 / divisor;
        let output = &mut sys.mem.ram[sys.gpu.pixel.copy_dst.value() as usize..];
        encode_depth_texture(pixels, cmd.depth_format(), stride, width, height, output);
    } else {
        let (sender, receiver) = oneshot::channel();
        let width = sys.gpu.pixel.copy_dimensions.width();
        let height = sys.gpu.pixel.copy_dimensions.height();

        // println!("COPYING COLOR TO {}", sys.gpu.pixel.copy_dst);
        sys.modules.render.exec(render::Action::ColorCopy {
            x: sys.gpu.pixel.copy_src.x().value(),
            y: sys.gpu.pixel.copy_src.y().value(),
            width,
            height,
            half: cmd.half(),
            clear: cmd.clear(),
            response: sender,
        });
        let pixels = receiver.recv().unwrap();
        // println!("RECEIVED DATA TO COPY TO {}", sys.gpu.pixel.copy_dst);

        let divisor = if cmd.half() { 2 } else { 1 };
        let stride = sys.gpu.pixel.copy_stride;
        let width = sys.gpu.pixel.copy_dimensions.width() as u32 / divisor;
        let height = sys.gpu.pixel.copy_dimensions.height() as u32 / divisor;
        let output = &mut sys.mem.ram[sys.gpu.pixel.copy_dst.value() as usize..];
        encode_color_texture(pixels, cmd.color_format(), stride, width, height, output);
    }
}
