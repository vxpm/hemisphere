use bitos::{BitUtils, bitos, integer::u3};
use common::util;
use glam::{Mat3, Mat4};
use strum::FromRepr;

use crate::{render::Action, system::System};

/// A transform unit register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum Reg {
    Error = 0x00,
    Diagnostics = 0x01,
    State0 = 0x02,
    State1 = 0x03,
    PowerSave = 0x04,
    ClipDisable = 0x05,
    Perf0 = 0x06,
    Perf1 = 0x07,
    InVertexSpec = 0x08,
    NumColors = 0x09,
    Ambient0 = 0x0A,
    Ambient1 = 0x0B,
    Material0 = 0x0C,
    Material1 = 0x0D,
    DiffuseControl = 0x0E,
    SpecularControl = 0x0F,
    DiffuseAlphaControl = 0x10,
    SpecularAlphaControl = 0x11,
    DualTextureTransform = 0x12,
    MatrixIndex0 = 0x18,
    MatrixIndex1 = 0x19,
    ViewportScaleX = 0x1A,
    ViewportScaleY = 0x1B,
    ViewportScaleZ = 0x1C,
    ViewportOffsetX = 0x1D,
    ViewportOffsetY = 0x1E,
    ViewportOffsetZ = 0x1F,
    ProjectionParam0 = 0x20,
    ProjectionParam1 = 0x21,
    ProjectionParam2 = 0x22,
    ProjectionParam3 = 0x23,
    ProjectionParam4 = 0x24,
    ProjectionParam5 = 0x25,
    ProjectionOrthographic = 0x26,
    TexgenCount = 0x3F,
    TexGen0 = 0x40,
    TexGen1 = 0x41,
    TexGen2 = 0x42,
    TexGen3 = 0x43,
    TexGen4 = 0x44,
    TexGen5 = 0x45,
    TexGen6 = 0x46,
    TexGen7 = 0x47,
    DualTexGen0 = 0x50,
    DualTexGen1 = 0x51,
    DualTexGen2 = 0x52,
    DualTexGen3 = 0x53,
    DualTexGen4 = 0x54,
    DualTexGen5 = 0x55,
    DualTexGen6 = 0x56,
    DualTexGen7 = 0x57,
}

impl Reg {
    pub fn is_viewport_dimensions(&self) -> bool {
        matches!(self, Reg::ViewportScaleX | Reg::ViewportScaleY)
    }

    pub fn is_projection_param(&self) -> bool {
        matches!(
            self,
            Reg::ProjectionParam0
                | Reg::ProjectionParam1
                | Reg::ProjectionParam2
                | Reg::ProjectionParam3
                | Reg::ProjectionParam4
                | Reg::ProjectionParam5
                | Reg::ProjectionOrthographic
        )
    }
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, Default)]
pub enum TexGenOutputKind {
    #[default]
    Vec2 = 0,
    Vec3 = 1,
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, Default)]
pub enum TexGenInputKind {
    #[default]
    AB11 = 0,
    ABC1 = 1,
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, Default)]
pub enum TexGenKind {
    #[default]
    Transform = 0b00,
    Emboss = 0b01,
    ColorDiffuse = 0b10,
    ColorSpecular = 0b11,
}

#[bitos(4)]
#[derive(Debug, Clone, Copy, Default)]
pub enum TexGenSource {
    #[default]
    Position = 0x0,
    Normal = 0x1,
    Color = 0x2,
    BinormalT = 0x3,
    BinormalB = 0x4,
    TexCoord0 = 0x5,
    TexCoord1 = 0x6,
    TexCoord2 = 0x7,
    TexCoord3 = 0x8,
    TexCoord4 = 0x9,
    TexCoord5 = 0xA,
    TexCoord6 = 0xB,
    TexCoord7 = 0xC,
    Reserved0 = 0xD,
    Reserved1 = 0xE,
    Reserved2 = 0xF,
}

#[bitos(32)]
#[derive(Debug, Clone, Default)]
pub struct TexGen {
    #[bits(1)]
    pub output_kind: TexGenOutputKind,
    #[bits(2)]
    pub input_kind: TexGenInputKind,
    #[bits(4..6)]
    pub kind: TexGenInputKind,
    #[bits(7..11)]
    pub source: TexGenSource,
    #[bits(12..15)]
    pub emboss_source: u3,
    #[bits(15..17)]
    pub emboss_light: u3,
}

#[derive(Debug, Default)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub far: f32,
    pub near: f32,
}

#[derive(Debug, Default)]
pub struct Internal {
    pub viewport: Viewport,
    pub projection_params: [f32; 6],
    pub projection_orthographic: bool,
}

/// Transform unit
#[derive(Debug)]
pub struct Interface {
    pub ram: Box<[u32; 0x1000]>,
    pub internal: Internal,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            ram: util::boxed_array(0),
            internal: Default::default(),
        }
    }
}

const Z_MAX: f32 = 16_777_215.0;

impl Interface {
    /// Returns the matrix at `index` in internal memory.
    pub fn matrix(&self, index: u8) -> Mat4 {
        let offset = 4 * index as usize;
        let data = &self.ram[offset..][..16];
        let m: &[f32] = zerocopy::transmute_ref!(data);

        Mat4::from_cols_array_2d(&[
            [m[0], m[1], m[2], m[3]],
            [m[4], m[5], m[6], m[7]],
            [m[8], m[9], m[10], m[11]],
            [0.0, 0.0, 0.0, 1.0],
        ])
        .transpose()
    }

    /// Returns the normal matrix at `index` in internal memory.
    pub fn normal_matrix(&self, index: u8) -> Mat3 {
        let offset = 4 * index as usize;
        let data = &self.ram[0x400 + offset..][..9];
        let m: &[f32] = zerocopy::transmute_ref!(data);

        Mat3::from_cols_array_2d(&[
            // this comment exists so rustfmt doesnt format this :)
            [m[0], m[1], m[2]],
            [m[3], m[4], m[5]],
            [m[6], m[7], m[8]],
        ])
        .transpose()
    }

    /// Returns the projection matrix.
    pub fn projection_matrix(&self) -> Mat4 {
        let p = &self.internal.projection_params;
        if self.internal.projection_orthographic {
            Mat4::from_cols_array_2d(&[
                [p[0], 0.0, 0.0, p[1]],
                [0.0, p[2], 0.0, p[3]],
                [0.0, 0.0, p[4], p[5]],
                [0.0, 0.0, 0.0, 1.0],
            ])
        } else {
            Mat4::from_cols_array_2d(&[
                [p[0], 0.0, p[1], 0.0],
                [0.0, p[2], p[3], 0.0],
                [0.0, 0.0, p[4], p[5]],
                [0.0, 0.0, -1.0, 0.0],
            ])
        }
        .transpose()
    }
}

impl System {
    /// Sets the value of an internal transform unit register.
    pub(super) fn xf_set(&mut self, reg: Reg, value: u32) {
        tracing::debug!("wrote {value:02X} to internal XF register {reg:?}");

        let xf = &mut self.gpu.transform.internal;
        match reg {
            Reg::ViewportScaleX => xf.viewport.width = f32::from_bits(value) * 2.0,
            Reg::ViewportScaleY => xf.viewport.height = f32::from_bits(value) * -2.0,
            Reg::ViewportScaleZ => xf.viewport.near = f32::from_bits(value) / Z_MAX,
            Reg::ViewportOffsetX => xf.viewport.center_x = f32::from_bits(value) - 342.0,
            Reg::ViewportOffsetY => xf.viewport.center_y = f32::from_bits(value) - 342.0,
            Reg::ViewportOffsetZ => xf.viewport.far = f32::from_bits(value) / Z_MAX,

            Reg::ProjectionParam0 => xf.projection_params[0] = f32::from_bits(value),
            Reg::ProjectionParam1 => xf.projection_params[1] = f32::from_bits(value),
            Reg::ProjectionParam2 => xf.projection_params[2] = f32::from_bits(value),
            Reg::ProjectionParam3 => xf.projection_params[3] = f32::from_bits(value),
            Reg::ProjectionParam4 => xf.projection_params[4] = f32::from_bits(value),
            Reg::ProjectionParam5 => xf.projection_params[5] = f32::from_bits(value),
            Reg::ProjectionOrthographic => xf.projection_orthographic = value != 0,

            _ => tracing::warn!("unimplemented write to internal XF register {reg:?}"),
        }

        if reg.is_viewport_dimensions() {
            self.config
                .renderer
                .exec(Action::SetViewport(crate::render::Viewport {
                    width: self.gpu.transform.internal.viewport.width.round() as u32,
                    height: self.gpu.transform.internal.viewport.height.round() as u32,
                }));
        }

        if reg.is_projection_param() {
            self.config.renderer.exec(Action::SetProjectionMatrix(
                self.gpu.transform.projection_matrix(),
            ));
        }
    }

    /// Writes to transform unit memory.
    pub(super) fn xf_write(&mut self, addr: u16, value: u32) {
        match addr {
            0x0000..0x0400 => self.gpu.transform.ram[addr as usize] = value,
            0x0400..0x0460 => self.gpu.transform.ram[addr as usize] = value.with_bits(0, 12, 0),
            0x0500..0x0600 => self.gpu.transform.ram[addr as usize] = value,
            0x0600..0x0680 => self.gpu.transform.ram[addr as usize] = value.with_bits(0, 12, 0),
            0x1000..0x1057 => {
                let register = addr as u8;
                let Some(register) = Reg::from_repr(register) else {
                    panic!("unknown XF register {register:02X}");
                };

                self.xf_set(register, value);
            }
            _ => tracing::debug!("writing to unknown XF memory"),
        }
    }
}
