pub mod attributes;

use crate::system::{
    System,
    gpu::{BypassReg, Gpu, command::attributes::AttributeDescriptor},
};
use attributes::VertexAttributeTable;
use bitos::{
    BitUtils, bitos,
    integer::{u3, u6},
};
use common::{
    Address, Primitive,
    bin::{BinRingBuffer, BinaryStream},
};
use strum::FromRepr;
use zerocopy::IntoBytes;

/// A command processor register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum Reg {
    Unknown = 0x20,

    MatIndexLow = 0x30,
    MatIndexHigh = 0x40,

    // VCD
    VcdLow = 0x50,
    VcdHigh = 0x60,

    // VAT
    Vat0A = 0x70,
    Vat1A = 0x71,
    Vat2A = 0x72,
    Vat3A = 0x73,
    Vat4A = 0x74,
    Vat5A = 0x75,
    Vat6A = 0x76,
    Vat7A = 0x77,

    Vat0B = 0x80,
    Vat1B = 0x81,
    Vat2B = 0x82,
    Vat3B = 0x83,
    Vat4B = 0x84,
    Vat5B = 0x85,
    Vat6B = 0x86,
    Vat7B = 0x87,

    Vat0C = 0x90,
    Vat1C = 0x91,
    Vat2C = 0x92,
    Vat3C = 0x93,
    Vat4C = 0x94,
    Vat5C = 0x95,
    Vat6C = 0x96,
    Vat7C = 0x97,

    // Array Base
    PositionPtr = 0xA0,
    NormalPtr = 0xA1,
    DiffusePtr = 0xA2,
    SpecularPtr = 0xA3,
    Tex0CoordPtr = 0xA4,
    Tex1CoordPtr = 0xA5,
    Tex2CoordPtr = 0xA6,
    Tex3CoordPtr = 0xA7,
    Tex4CoordPtr = 0xA8,
    Tex5CoordPtr = 0xA9,
    Tex6CoordPtr = 0xAA,
    Tex7CoordPtr = 0xAB,
    GpArr0Ptr = 0xAC,
    GpArr1Ptr = 0xAD,
    GpArr2Ptr = 0xAE,
    GpArr3Ptr = 0xAF,

    // Array Stride
    PositionStride = 0xB0,
    NormalStride = 0xB1,
    DiffuseStride = 0xB2,
    SpecularStride = 0xB3,
    Tex0CoordStride = 0xB4,
    Tex1CoordStride = 0xB5,
    Tex2CoordStride = 0xB6,
    Tex3CoordStride = 0xB7,
    Tex4CoordStride = 0xB8,
    Tex5CoordStride = 0xB9,
    Tex6CoordStride = 0xBA,
    Tex7CoordStride = 0xBB,
    GpArr0Stride = 0xBC,
    GpArr1Stride = 0xBD,
    GpArr2Stride = 0xBE,
    GpArr3Stride = 0xBF,
}

#[bitos(5)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Operation {
    #[default]
    NOP = 0b0000_0,
    SetCP = 0b0000_1,
    SetXF = 0b0001_0,
    IndexedSetXFA = 0b0010_0,
    IndexedSetXFB = 0b0010_1,
    IndexedSetXFC = 0b0011_0,
    IndexedSetXFD = 0b0011_1,
    Call = 0b0100_0,
    InvalidateVertexCache = 0b0100_1,
    SetBP = 0b0110_0,
    DrawQuads = 0b1000_0,
    DrawTriangles = 0b1001_0,
    DrawTriangleStrip = 0b1001_1,
    DrawTriangleFan = 0b1010_0,
    DrawLines = 0b1010_1,
    DrawLineStrip = 0b1011_0,
    DrawPoints = 0b1011_1,
}

#[bitos(8)]
#[derive(Debug)]
pub struct Opcode {
    #[bits(0..3)]
    pub vat_index: u3,
    #[bits(3..8)]
    pub operation: Option<Operation>,
}

#[derive(Debug)]
pub enum Command {
    Nop,
    InvalidateVertexCache,
    SetCP {
        register: Reg,
        value: u32,
    },
    SetBP {
        register: BypassReg,
        value: u32,
    },
    SetXF {
        start: u16,
        length: u32,
        values: Vec<u32>,
    },
    DrawTriangles {
        vertex_attributes: VertexAttributeStream,
    },
}

/// CP status register
#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Status {
    #[bits(0)]
    pub fifo_overflow: bool,
    #[bits(1)]
    pub fifo_underflow: bool,
    #[bits(2)]
    pub read_idle: bool,
    #[bits(3)]
    pub write_idle: bool,
    #[bits(4)]
    pub breakpoint_interrupt: bool,
}

/// CP control register
#[bitos(16)]
#[derive(Debug, Clone, Copy)]
pub struct Control {
    #[bits(0)]
    pub fifo_read_enable: bool,
    #[bits(1)]
    pub fifo_breakpoint_enable: bool,
    #[bits(2)]
    pub fifo_overflow_interrupt_enable: bool,
    #[bits(3)]
    pub fifo_underflow_interrupt_enable: bool,
    #[bits(4)]
    pub linked_mode: bool,
    #[bits(5)]
    pub fifo_breakpoint_interrupt_enable: bool,
}

impl Default for Control {
    fn default() -> Self {
        Self::from_bits(0).with_linked_mode(true)
    }
}

#[derive(Debug, Default)]
pub struct Fifo {
    pub start: Address,
    pub end: Address,
    pub high_mark: u32,
    pub low_mark: u32,
    pub count: u32,
    pub write_ptr: Address,
    pub read_ptr: Address,
}

#[bitos[2]]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttributeMode {
    /// Not present
    #[default]
    None = 0b00,
    /// Directly in the vertex attribute stream
    Direct = 0b01,
    /// Indirectly through a 8 bit index in the vertex attribute stream
    Index8 = 0b10,
    /// Indirectly through a 16 bit index in the vertex attribute stream
    Index16 = 0b11,
}

impl AttributeMode {
    pub fn present(self) -> bool {
        self != AttributeMode::None
    }

    pub fn size(self) -> Option<u32> {
        match self {
            Self::None => Some(0),
            Self::Direct => None,
            Self::Index8 => Some(1),
            Self::Index16 => Some(2),
        }
    }
}

/// Describes which attributes are present in the vertices of primitives and how they are present.
#[bitos(64)]
#[derive(Debug, Clone, Default)]
pub struct VertexDescriptor {
    /// Whether the position/normal matrix index is present.
    #[bits(0)]
    pub pos_mat_index: bool,
    /// Whether the texture coordinate matrix N index is present.
    #[bits(1..9)]
    pub tex_coord_mat_index: [bool; 8],
    /// Whether the position attribute is present.
    #[bits(9..11)]
    pub position: AttributeMode,
    /// Whether the normal attribute is present.
    #[bits(11..13)]
    pub normal: AttributeMode,
    /// Whether the diffuse color attribute is present.
    #[bits(13..15)]
    pub diffuse: AttributeMode,
    /// Whether the specular color attribute is present.
    #[bits(15..17)]
    pub specular: AttributeMode,
    /// Whether the texture coordinate N attribute is present.
    #[bits(32..48)]
    pub tex_coord: [AttributeMode; 8],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ArrayDescriptor {
    pub address: Address,
    pub stride: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Arrays {
    pub position: ArrayDescriptor,
    pub diffuse: ArrayDescriptor,
}

#[bitos(64)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MatrixIndices {
    #[bits(0..6)]
    pub view: u6,
    #[bits(6..54)]
    pub tex: [u6; 8],
}

#[derive(Debug, Clone, Default)]
pub struct Internal {
    pub vertex_descriptor: VertexDescriptor,
    pub vertex_attr_tables: [VertexAttributeTable; 8],
    pub arrays: Arrays,
    pub mat_indices: MatrixIndices,
}

impl Internal {
    pub fn set(&mut self, reg: Reg, value: u32) {
        match reg {
            Reg::MatIndexLow => value.write_ne_bytes(&mut self.mat_indices.as_mut_bytes()[0..4]),
            Reg::MatIndexHigh => value.write_ne_bytes(&mut self.mat_indices.as_mut_bytes()[4..8]),

            Reg::VcdLow => value.write_ne_bytes(&mut self.vertex_descriptor.as_mut_bytes()[0..4]),
            Reg::VcdHigh => value.write_ne_bytes(&mut self.vertex_descriptor.as_mut_bytes()[4..8]),

            Reg::Vat0A => value.write_ne_bytes(self.vertex_attr_tables[0].a.as_mut_bytes()),
            Reg::Vat1A => value.write_ne_bytes(self.vertex_attr_tables[1].a.as_mut_bytes()),
            Reg::Vat2A => value.write_ne_bytes(self.vertex_attr_tables[2].a.as_mut_bytes()),
            Reg::Vat3A => value.write_ne_bytes(self.vertex_attr_tables[3].a.as_mut_bytes()),
            Reg::Vat4A => value.write_ne_bytes(self.vertex_attr_tables[4].a.as_mut_bytes()),
            Reg::Vat5A => value.write_ne_bytes(self.vertex_attr_tables[5].a.as_mut_bytes()),
            Reg::Vat6A => value.write_ne_bytes(self.vertex_attr_tables[6].a.as_mut_bytes()),
            Reg::Vat7A => value.write_ne_bytes(self.vertex_attr_tables[7].a.as_mut_bytes()),

            Reg::Vat0B => value.write_ne_bytes(self.vertex_attr_tables[0].b.as_mut_bytes()),
            Reg::Vat1B => value.write_ne_bytes(self.vertex_attr_tables[1].b.as_mut_bytes()),
            Reg::Vat2B => value.write_ne_bytes(self.vertex_attr_tables[2].b.as_mut_bytes()),
            Reg::Vat3B => value.write_ne_bytes(self.vertex_attr_tables[3].b.as_mut_bytes()),
            Reg::Vat4B => value.write_ne_bytes(self.vertex_attr_tables[4].b.as_mut_bytes()),
            Reg::Vat5B => value.write_ne_bytes(self.vertex_attr_tables[5].b.as_mut_bytes()),
            Reg::Vat6B => value.write_ne_bytes(self.vertex_attr_tables[6].b.as_mut_bytes()),
            Reg::Vat7B => value.write_ne_bytes(self.vertex_attr_tables[7].b.as_mut_bytes()),

            Reg::Vat0C => value.write_ne_bytes(self.vertex_attr_tables[0].c.as_mut_bytes()),
            Reg::Vat1C => value.write_ne_bytes(self.vertex_attr_tables[1].c.as_mut_bytes()),
            Reg::Vat2C => value.write_ne_bytes(self.vertex_attr_tables[2].c.as_mut_bytes()),
            Reg::Vat3C => value.write_ne_bytes(self.vertex_attr_tables[3].c.as_mut_bytes()),
            Reg::Vat4C => value.write_ne_bytes(self.vertex_attr_tables[4].c.as_mut_bytes()),
            Reg::Vat5C => value.write_ne_bytes(self.vertex_attr_tables[5].c.as_mut_bytes()),
            Reg::Vat6C => value.write_ne_bytes(self.vertex_attr_tables[6].c.as_mut_bytes()),
            Reg::Vat7C => value.write_ne_bytes(self.vertex_attr_tables[7].c.as_mut_bytes()),

            Reg::PositionPtr => value.write_ne_bytes(self.arrays.position.address.as_mut_bytes()),
            Reg::DiffusePtr => value.write_ne_bytes(self.arrays.diffuse.address.as_mut_bytes()),

            Reg::PositionStride => value.write_ne_bytes(self.arrays.position.stride.as_mut_bytes()),
            Reg::DiffuseStride => value.write_ne_bytes(self.arrays.diffuse.stride.as_mut_bytes()),

            _ => tracing::warn!("unimplemented write to internal CP register {reg:?}"),
        }
    }

    pub fn vertex_size(&self, vat: u8) -> u32 {
        let vat = vat as usize;

        let mut size = 0;
        if self.vertex_descriptor.pos_mat_index() {
            size += 1;
        }

        for i in 0..8 {
            if self.vertex_descriptor.tex_coord_mat_index_at(i).unwrap() {
                size += 1;
            }
        }

        size += self
            .vertex_descriptor
            .position()
            .size()
            .unwrap_or_else(|| self.vertex_attr_tables[vat].a.position().size());

        size += self
            .vertex_descriptor
            .normal()
            .size()
            .unwrap_or_else(|| self.vertex_attr_tables[vat].a.normal().size());

        size += self
            .vertex_descriptor
            .diffuse()
            .size()
            .unwrap_or_else(|| self.vertex_attr_tables[vat].a.diffuse().size());

        size += self
            .vertex_descriptor
            .specular()
            .size()
            .unwrap_or_else(|| self.vertex_attr_tables[vat].a.specular().size());

        for i in 0..8 {
            size += self
                .vertex_descriptor
                .tex_coord_at(i)
                .unwrap()
                .size()
                .unwrap_or_else(|| self.vertex_attr_tables[vat].tex(i).unwrap().size());
        }

        size
    }
}

#[derive(Debug, Clone)]
pub struct VertexAttributeStream {
    table: u8,
    count: u16,
    attributes: Vec<u8>,
}

impl VertexAttributeStream {
    pub fn table_index(&self) -> usize {
        self.table as usize
    }

    pub fn count(&self) -> u16 {
        self.count
    }

    pub fn data(&self) -> &[u8] {
        &self.attributes
    }

    pub fn stride(&self) -> usize {
        self.attributes.len() / self.count as usize
    }
}

/// CP interface
#[derive(Debug, Default)]
pub struct Interface {
    pub status: Status,
    pub control: Control,
    pub fifo: Fifo,
    pub internal: Internal,
    pub queue: BinRingBuffer,
}

impl Interface {
    /// Write a value to the clear register.
    pub fn write_clear(&mut self, value: u16) {
        if value.bit(0) {
            self.status.set_fifo_overflow(false);
        }

        if value.bit(1) {
            self.status.set_fifo_underflow(false);
        }
    }

    /// Updates the FIFO count.
    pub fn update_count(&mut self) {
        let count = if self.fifo.write_ptr >= self.fifo.start {
            self.fifo.write_ptr - self.fifo.read_ptr
        } else {
            let start = self.fifo.write_ptr - self.fifo.start;
            let end = self.fifo.end - self.fifo.read_ptr;
            start + end
        };

        assert!(count >= 0);
        self.fifo.count = count as u32;
    }

    /// Signals a value has been pushed to the CP FIFO.
    pub fn fifo_push(&mut self) {
        self.fifo.write_ptr += 1;

        if self.fifo.write_ptr > self.fifo.end {
            self.fifo.write_ptr = self.fifo.start;
        }

        self.update_count();
    }

    /// Signals a value has been popped from the CP FIFO.
    pub fn fifo_pop(&mut self) {
        self.fifo.read_ptr += 1;

        if self.fifo.read_ptr > self.fifo.end {
            self.fifo.read_ptr = self.fifo.start;
        }

        self.update_count();
    }
}

impl Gpu {
    /// Reads a command from the command queue.
    pub fn read_command(&mut self) -> Option<Command> {
        let mut reader = self.command.queue.reader();

        let opcode = Opcode::from_bits(reader.read_be()?);
        let command = match opcode.operation().unwrap() {
            Operation::NOP => Command::Nop,
            Operation::SetCP => {
                let register = reader.read_be::<u8>()?;
                let value = reader.read_be::<u32>()?;

                let Some(register) = Reg::from_repr(register) else {
                    panic!("unknown cp register {register:02X}");
                };

                Command::SetCP { register, value }
            }
            Operation::SetXF => {
                let length = reader.read_be::<u16>()? as u32 + 1;
                if reader.remaining() < 4 * length as usize {
                    return None;
                }

                let start = reader.read_be::<u16>()?;
                let mut values = Vec::with_capacity(length as usize);
                for _ in 0..length {
                    values.push(reader.read_be::<u32>()?);
                }

                Command::SetXF {
                    start,
                    length,
                    values,
                }
            }
            Operation::IndexedSetXFA => todo!(),
            Operation::IndexedSetXFB => todo!(),
            Operation::IndexedSetXFC => todo!(),
            Operation::IndexedSetXFD => todo!(),
            Operation::Call => todo!(),
            Operation::InvalidateVertexCache => Command::InvalidateVertexCache,
            Operation::SetBP => {
                let register = reader.read_be::<u8>()?;
                let value = u32::from_be_bytes([
                    0,
                    reader.read_be::<u8>()?,
                    reader.read_be::<u8>()?,
                    reader.read_be::<u8>()?,
                ]);

                let Some(register) = BypassReg::from_repr(register) else {
                    panic!("unknown bypass register {register:02X}");
                };

                Command::SetBP { register, value }
            }
            Operation::DrawQuads => todo!(),
            Operation::DrawTriangles => {
                let vertex_count = reader.read_be::<u16>()?;
                let vertex_size = self
                    .command
                    .internal
                    .vertex_size(opcode.vat_index().value());

                let attribute_stream_size = vertex_count as usize * vertex_size as usize;
                if reader.remaining() < attribute_stream_size {
                    return None;
                }

                let vertex_attributes = reader.read_bytes(attribute_stream_size)?;
                let vertex_attributes = VertexAttributeStream {
                    table: opcode.vat_index().value(),
                    count: vertex_count,
                    attributes: vertex_attributes,
                };

                Command::DrawTriangles { vertex_attributes }
            }
            Operation::DrawTriangleStrip => todo!(),
            Operation::DrawTriangleFan => todo!(),
            Operation::DrawLines => todo!(),
            Operation::DrawLineStrip => todo!(),
            Operation::DrawPoints => todo!(),
        };

        reader.finish();
        Some(command)
    }
}

/// Command Processor
impl System {
    /// Pops a value from the CP FIFO in memory.
    fn cp_fifo_pop(&mut self) -> u8 {
        assert!(self.gpu.command.fifo.count > 0);

        let data = self.read::<u8>(self.gpu.command.fifo.read_ptr);
        self.gpu.command.fifo_pop();

        data
    }

    /// Process CP commands until the queue is either empty or incomplete.
    pub fn cp_process(&mut self) {
        loop {
            if self.gpu.command.queue.is_empty() {
                return;
            }

            let Some(cmd) = self.gpu.read_command() else {
                return;
            };

            tracing::debug!("{:02X?}", cmd);

            match cmd {
                Command::Nop => (),
                Command::InvalidateVertexCache => (),
                Command::SetCP { register, value } => {
                    self.gpu.command.internal.set(register, value)
                }
                Command::SetBP { .. } => (),
                Command::SetXF { start, values, .. } => {
                    for (offset, value) in values.into_iter().enumerate() {
                        self.gpu.transform.write(start + offset as u16, value);
                    }
                }
                Command::DrawTriangles { vertex_attributes } => {
                    self.gx_draw_triangle(vertex_attributes);
                }
            }
        }
    }

    /// Consumes commands available in the CP FIFO and processes them.
    pub fn cp_update(&mut self) {
        while self.gpu.command.fifo.count > 0 {
            let data = self.cp_fifo_pop();
            self.gpu.command.queue.push_be(data);
        }

        self.cp_process();
    }
}
