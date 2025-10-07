use crate::system::gpu::{AttributeKind, BypassReg};
use bitos::{BitUtils, bitos, integer::u3};
use common::{Address, util::DataStream};
use strum::FromRepr;

/// A command processor register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum Reg {
    Unknown = 0x20,

    TexMatIndexA = 0x30,
    TexMatIndexB = 0x40,

    // VCD
    VcdLow = 0x50,
    VcdHigh = 0x60,

    // VAT
    Vat0Format0 = 0x70,
    Vat0Format1 = 0x71,
    Vat0Format2 = 0x72,
    Vat0Format3 = 0x73,
    Vat0Format4 = 0x74,
    Vat0Format5 = 0x75,
    Vat0Format6 = 0x76,
    Vat0Format7 = 0x77,

    Vat1Format0 = 0x80,
    Vat1Format1 = 0x81,
    Vat1Format2 = 0x82,
    Vat1Format3 = 0x83,
    Vat1Format4 = 0x84,
    Vat1Format5 = 0x85,
    Vat1Format6 = 0x86,
    Vat1Format7 = 0x87,

    Vat2Format0 = 0x90,
    Vat2Format1 = 0x91,
    Vat2Format2 = 0x92,
    Vat2Format3 = 0x93,
    Vat2Format4 = 0x94,
    Vat2Format5 = 0x95,
    Vat2Format6 = 0x96,
    Vat2Format7 = 0x97,

    // Array Base
    VerticesPtr = 0xA0,
    NormalsPtr = 0xA1,
    Color0Ptr = 0xA2,
    Color1Ptr = 0xA3,
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
    VerticesStride = 0xB0,
    NormalsStride = 0xB1,
    Color0Stride = 0xB2,
    Color1Stride = 0xB3,
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
        vat_index: u8,
        vertex_count: u16,
    },
}

impl Command {
    /// Reads a command from the given data stream.
    pub fn read(stream: &mut DataStream) -> Option<Self> {
        let mut reader = stream.read();

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
                Command::DrawTriangles {
                    vat_index: opcode.vat_index().value(),
                    vertex_count,
                }
            }
            Operation::DrawTriangleStrip => todo!(),
            Operation::DrawTriangleFan => todo!(),
            Operation::DrawLines => todo!(),
            Operation::DrawLineStrip => todo!(),
            Operation::DrawPoints => todo!(),
        };

        reader.consume();
        Some(command)
    }
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
    pub position: AttributeKind,
    /// Whether the normal attribute is present.
    #[bits(11..13)]
    pub normal: AttributeKind,
    /// Whether the color N attribute is present.
    #[bits(13..17)]
    pub color: [AttributeKind; 2],
    /// Whether the texture coordinate N attribute is present.
    #[bits(17..33)]
    pub tex_coord: [AttributeKind; 8],
}

/// CP interface
#[derive(Debug, Default)]
pub struct Interface {
    pub status: Status,
    pub control: Control,
    pub fifo: Fifo,
    pub vertex_descriptor: VertexDescriptor,
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
