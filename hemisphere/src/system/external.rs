use crate::system::System;
use bitos::{
    bitos,
    integer::{u2, u3},
};
use common::{Address, Primitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device0 {
    MemoryCardA,
    IplRtcSram,
    SerialPort1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device1 {
    MemoryCardB,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device2 {
    AD16,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Parameter {
    #[bits(0)]
    pub device_interrupt_mask: bool,
    #[bits(1)]
    pub device_interrupt: bool,
    #[bits(2)]
    pub transfer_interrupt_mask: bool,
    #[bits(3)]
    pub transfer_interrupt: bool,
    #[bits(4..7)]
    pub clock_multiplier: u3,
    #[bits(7..10)]
    pub device_select: u3,
    #[bits(10)]
    pub attach_interrupt_mask: bool,
    #[bits(11)]
    pub attach_interrupt: bool,
    #[bits(12)]
    pub device_connected: bool,
}

impl Parameter {
    pub fn write(&mut self, value: Parameter) {
        self.set_device_interrupt_mask(value.device_interrupt_mask());
        self.set_device_interrupt(self.device_interrupt() & !value.device_interrupt());
        self.set_transfer_interrupt_mask(value.transfer_interrupt_mask());
        self.set_transfer_interrupt(self.transfer_interrupt() & !value.transfer_interrupt());

        self.set_clock_multiplier(value.clock_multiplier());
        self.set_device_select(value.device_select());

        self.set_attach_interrupt_mask(value.attach_interrupt_mask());
        self.set_attach_interrupt(self.attach_interrupt() & !value.attach_interrupt());
    }

    pub fn device0(&self) -> Option<Device0> {
        Some(match self.device_select().value() {
            0b001 => Device0::MemoryCardA,
            0b010 => Device0::IplRtcSram,
            0b100 => Device0::SerialPort1,
            _ => return None,
        })
    }

    pub fn device1(&self) -> Option<Device1> {
        Some(match self.device_select().value() {
            0b001 => Device1::MemoryCardB,
            _ => return None,
        })
    }

    pub fn device2(&self) -> Option<Device2> {
        Some(match self.device_select().value() {
            0b001 => Device2::AD16,
            _ => return None,
        })
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    Read = 0b00,
    Write = 0b01,
    ReadWrite = 0b10,
    Reserved = 0b11,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Control {
    #[bits(0)]
    pub transfer_ongoing: bool,
    #[bits(1)]
    pub dma: bool,
    #[bits(2..4)]
    pub transfer_mode: TransferMode,
    #[bits(4..6)]
    pub imm_length_minus_one: u2,
}

impl Control {
    pub fn imm_length(&self) -> u32 {
        self.imm_length_minus_one().value() as u32 + 1
    }
}

#[derive(Debug, Clone, Default)]
pub enum Ch0State {
    #[default]
    Idle,
}

#[derive(Debug, Clone, Default)]
pub struct Channel0 {
    pub ipl_base: u32,
    pub rtc: u32,
    pub state: Ch0State,

    pub parameter: Parameter,
    pub control: Control,
    pub dma_base: Address,
    pub dma_length: u32,
    pub immediate: u32,
}

#[derive(Default, Debug, Clone)]
pub struct Channel1 {
    pub parameter: Parameter,
    pub control: Control,
    pub dma_base: Address,
    pub dma_length: u32,
    pub immediate: u32,
}

#[derive(Default, Debug, Clone)]
pub struct Channel2 {
    pub parameter: Parameter,
    pub control: Control,
    pub dma_base: Address,
    pub dma_length: u32,
    pub immediate: u32,
}

#[derive(Default)]
pub struct Interface {
    pub channel0: Channel0,
    pub channel1: Channel0,
    pub channel2: Channel0,
}

impl System {
    fn exi_ipl_transfer(&mut self) {
        if !self.external.channel0.control.dma() {
            self.external.channel0.ipl_base = self.external.channel0.immediate >> 6;
            tracing::debug!("set IPL base to 0x{:08X}", self.external.channel0.ipl_base);
            return;
        }

        let ram_base = self.external.channel0.dma_base.value() as usize;
        let ipl_base = self.external.channel0.ipl_base as usize;
        let length = self.external.channel0.dma_length as usize;
        tracing::debug!(
            "IPL ROM DMA: 0x{:08X} bytes from IPL 0x{:08X} to RAM 0x{:08X}",
            length,
            ipl_base,
            ram_base
        );

        self.mem.ram[ram_base..][..length].copy_from_slice(&self.mem.ipl[ipl_base..][..length]);
    }

    fn exi_sram_transfer(&mut self) {
        let sram_base = ((self.external.channel0.immediate & !0xA000_0000) >> 6) as usize;
        tracing::debug!("SRAM TRANSFER");

        if !self.external.channel0.control.dma() {
            self.external.channel0.immediate = u32::read_be_bytes(&self.mem.sram[sram_base..]);
            todo!();
            return;
        }

        let ram_base = self.external.channel0.dma_base.value() as usize;
        let length = self.external.channel0.dma_length as usize;
        tracing::debug!(
            "SRAM DMA: 0x{:08X} bytes from SRAM 0x{:08X} to RAM 0x{:08X}",
            length,
            sram_base,
            ram_base
        );

        self.mem.ram[ram_base..][..length].copy_from_slice(&self.mem.sram[sram_base..][..length]);
    }

    fn exi_ipl_rtc_sram_transfer(&mut self) {
        match self.external.channel0.clone().state {
            Ch0State::Idle => {
                // new transfer
                match self.external.channel0.clone().immediate {
                    0x0000_0000..0x2000_0000 => self.exi_ipl_transfer(),
                    0x2000_0000 => {
                        tracing::debug!("RTC read: 0x{:08X}", self.external.channel0.rtc);
                        assert!(!self.external.channel0.control.dma());
                        self.external.channel0.immediate = self.external.channel0.rtc;
                    }
                    0x2000_0100..0x2000_1100 => {
                        self.exi_sram_transfer();
                    }
                    0xA000_0000 => {
                        tracing::debug!("RTC write: 0x{:08X}", self.external.channel0.immediate);
                        assert!(!self.external.channel0.control.dma());
                        self.external.channel0.rtc = self.external.channel0.immediate;
                    }
                    _ => todo!(),
                }
            }
        }

        self.external.channel0.control.set_transfer_ongoing(false);
    }

    pub fn exi_channel0_transfer(&mut self) {
        match self.external.channel0.parameter.device0().unwrap() {
            Device0::IplRtcSram => {
                self.exi_ipl_rtc_sram_transfer();
            }
            Device0::SerialPort1 => {
                // no ethernet adapter
                tracing::debug!("SP1 read - ignoring");
                self.external.channel0.immediate = 0;
                self.external.channel0.control.set_transfer_ongoing(false);
            }
            _ => todo!(),
        }
    }

    pub fn exi_update(&mut self) {
        if self.external.channel0.control.transfer_ongoing() {
            self.exi_channel0_transfer();
        }

        if self.external.channel2.control.transfer_ongoing() {
            // self.exi_channel2_transfer();
        }
    }
}
