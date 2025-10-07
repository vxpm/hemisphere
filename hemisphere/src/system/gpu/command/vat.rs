use bitos::{bitos, integer::u5};

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionKind {
    /// Two components (x, y).
    #[default]
    Vec2 = 0b0,
    /// Three components (x, y, z).
    Vec3 = 0b1,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoordsFormat {
    #[default]
    U8 = 0b000,
    I8 = 0b001,
    U16 = 0b010,
    I16 = 0b011,
    F32 = 0b100,
}

#[bitos(9)]
#[derive(Debug, Clone, Default)]
pub struct PositionAttribute {
    #[bits(0)]
    pub kind: PositionKind,
    #[bits(1..4)]
    pub format: Option<CoordsFormat>,
    #[bits(4..9)]
    pub shift: u5,
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NormalKind {
    /// Three normals.
    #[default]
    N3 = 0b0,
    /// Nine normals.
    N9 = 0b1,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NormalFormat {
    #[default]
    I8 = 0b001,
    I16 = 0b011,
    F32 = 0b100,
}

#[bitos(4)]
#[derive(Debug, Clone, Default)]
pub struct NormalAttribute {
    #[bits(0)]
    pub kind: NormalKind,
    #[bits(1..4)]
    pub format: Option<NormalFormat>,
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorKind {
    /// Three components (r, g, b).
    #[default]
    Rgb = 0b0,
    /// Four components (r, g, b, a).
    Rgba = 0b1,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorFormat {
    #[default]
    Rgb565 = 0b000,
    Rgb888 = 0b001,
    Rgb888x = 0b010,
    Rgba4444 = 0b011,
    Rgba6666 = 0b100,
    Rgba8888 = 0b101,
}

#[bitos(4)]
#[derive(Debug, Clone, Default)]
pub struct ColorAttribute {
    #[bits(0)]
    pub kind: ColorKind,
    #[bits(1..4)]
    pub format: Option<ColorFormat>,
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TexCoordsKind {
    /// One components (s).
    #[default]
    Vec1 = 0b0,
    /// Two components (s, t).
    Vec2 = 0b1,
}

#[bitos(9)]
#[derive(Debug, Clone, Default)]
pub struct TexCoordsAttribute {
    #[bits(0)]
    pub kind: TexCoordsKind,
    #[bits(1..4)]
    pub format: Option<CoordsFormat>,
    #[bits(4..9)]
    pub shift: u5,
}

#[bitos(32)]
#[derive(Debug, Clone, Default)]
pub struct VertexAttributeTableA {
    #[bits(0..9)]
    pub position: PositionAttribute,
    #[bits(9..13)]
    pub normal: NormalAttribute,
    #[bits(13..17)]
    pub diffuse: ColorAttribute,
    #[bits(17..21)]
    pub specular: ColorAttribute,
    #[bits(21..30)]
    pub tex0: TexCoordsAttribute,
    #[bits(30)]
    pub byte_dequant: bool,
    #[bits(31)]
    pub normal_index: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Default)]
pub struct VertexAttributeTableB {
    #[bits(0..27)]
    pub tex1to3: [TexCoordsAttribute; 3],

    #[bits(27)]
    pub tex4_kind: TexCoordsKind,
    #[bits(28..31)]
    pub tex4_format: Option<CoordsFormat>,

    #[bits(31)]
    pub vcache_enhance: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Default)]
pub struct VertexAttributeTableC {
    #[bits(0..5)]
    pub tex4_shift: u5,
    #[bits(5..32)]
    pub tex5to7: [TexCoordsAttribute; 3],
}

#[derive(Debug, Clone, Default)]
pub struct VertexAttributeTable {
    pub a: VertexAttributeTableA,
    pub b: VertexAttributeTableB,
    pub c: VertexAttributeTableC,
}
