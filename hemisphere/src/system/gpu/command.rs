use bitos::{BitUtils, bitos, integer::u3};
use common::{Address, util::DataStream};

use crate::system::gpu::{BypassReg, CpReg};

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
        register: CpReg,
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

                let Some(register) = CpReg::from_repr(register) else {
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

/// CP interface
#[derive(Debug, Default)]
pub struct Interface {
    pub status: Status,
    pub control: Control,
    pub fifo_start: Address,
    pub fifo_end: Address,
    pub fifo_high_mark: u32,
    pub fifo_low_mark: u32,
    pub fifo_count: u32,
    pub fifo_write_ptr: Address,
    pub fifo_read_ptr: Address,
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
        let count = if self.fifo_write_ptr >= self.fifo_start {
            self.fifo_write_ptr - self.fifo_read_ptr
        } else {
            let start = self.fifo_write_ptr - self.fifo_start;
            let end = self.fifo_end - self.fifo_read_ptr;
            start + end
        };

        assert!(count >= 0);
        self.fifo_count = count as u32;
    }

    /// Signals a value has been pushed to the CP FIFO.
    pub fn fifo_push(&mut self) {
        self.fifo_write_ptr += 1;

        if self.fifo_write_ptr > self.fifo_end {
            self.fifo_write_ptr = self.fifo_start;
        }

        self.update_count();
    }

    /// Signals a value has been popped from the CP FIFO.
    pub fn fifo_pop(&mut self) {
        self.fifo_read_ptr += 1;

        if self.fifo_read_ptr > self.fifo_end {
            self.fifo_read_ptr = self.fifo_start;
        }

        self.update_count();
    }
}
