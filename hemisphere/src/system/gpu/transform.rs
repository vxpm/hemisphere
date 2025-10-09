use bitos::BitUtils;
use common::util;
use glam::Mat4;
use strum::FromRepr;

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
    Tex0 = 0x40,
    Tex1 = 0x41,
    Tex2 = 0x42,
    Tex3 = 0x43,
    Tex4 = 0x44,
    Tex5 = 0x45,
    Tex6 = 0x46,
    Tex7 = 0x47,
    DualTex0 = 0x50,
    DualTex1 = 0x51,
    DualTex2 = 0x52,
    DualTex3 = 0x53,
    DualTex4 = 0x54,
    DualTex5 = 0x55,
    DualTex6 = 0x56,
    DualTex7 = 0x57,
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

const Z_MAX: f32 = 16777215.0;

impl Interface {
    pub fn set(&mut self, reg: Reg, value: u32) {
        tracing::debug!("wrote {value:02X} to xf {reg:?}");

        match reg {
            Reg::ViewportScaleX => self.internal.viewport.width = f32::from_bits(value) * 2.0,
            Reg::ViewportScaleY => self.internal.viewport.height = f32::from_bits(value) * -2.0,
            Reg::ViewportScaleZ => self.internal.viewport.near = f32::from_bits(value) / Z_MAX,
            Reg::ViewportOffsetX => self.internal.viewport.center_x = f32::from_bits(value) - 342.0,
            Reg::ViewportOffsetY => self.internal.viewport.center_y = f32::from_bits(value) - 342.0,
            Reg::ViewportOffsetZ => self.internal.viewport.far = f32::from_bits(value) / Z_MAX,

            Reg::ProjectionParam0 => self.internal.projection_params[0] = f32::from_bits(value),
            Reg::ProjectionParam1 => self.internal.projection_params[1] = f32::from_bits(value),
            Reg::ProjectionParam2 => self.internal.projection_params[2] = f32::from_bits(value),
            Reg::ProjectionParam3 => self.internal.projection_params[3] = f32::from_bits(value),
            Reg::ProjectionParam4 => self.internal.projection_params[4] = f32::from_bits(value),
            Reg::ProjectionParam5 => self.internal.projection_params[5] = f32::from_bits(value),
            Reg::ProjectionOrthographic => self.internal.projection_orthographic = value != 0,

            _ => tracing::warn!("unimplemented write to internal XF register {reg:?}"),
        }
    }

    pub fn write(&mut self, addr: u16, value: u32) {
        match addr {
            0x0000..0x0400 => self.ram[addr as usize] = value,
            0x0400..0x0460 => self.ram[addr as usize] = value.with_bits(0, 12, 0),
            0x0500..0x0600 => self.ram[addr as usize] = value,
            0x0600..0x0680 => self.ram[addr as usize] = value.with_bits(0, 12, 0),
            0x1000..0x1057 => {
                let register = addr as u8;
                let Some(register) = Reg::from_repr(register) else {
                    panic!("unknown xf register {register:02X}");
                };

                self.set(register, value);
            }
            _ => tracing::debug!("writing to unknown XF memory"),
        }
    }

    /// Returns the matrix at `index` in internal memory.
    pub fn matrix(&self, index: u8) -> Mat4 {
        let offset = 4 * index as usize;
        let data = &self.ram[offset..16];
        let m: &[f32] = zerocopy::transmute_ref!(data);

        Mat4::from_cols_array_2d(&[
            [m[0], m[1], m[2], m[3]],
            [m[4], m[5], m[6], m[7]],
            [m[8], m[9], m[10], m[11]],
            [0.0, 0.0, 0.0, 1.0],
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
