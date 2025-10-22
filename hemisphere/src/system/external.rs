use crate::system::System;
use bitos::{
    bitos,
    integer::{u2, u3},
};
use common::{Address, Primitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    MemoryCardA,
    IplRtcSram,
    Uart,
    MemoryCardB,
    AD16,
    SerialPort1,
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

    pub fn device0(&self) -> Option<Device> {
        Some(match self.device_select().value() {
            0b001 => Device::MemoryCardA,
            0b010 => Device::IplRtcSram,
            0b100 => Device::SerialPort1,
            _ => return None,
        })
    }

    pub fn device1(&self) -> Option<Device> {
        Some(match self.device_select().value() {
            0b001 => Device::MemoryCardB,
            _ => return None,
        })
    }

    pub fn device2(&self) -> Option<Device> {
        Some(match self.device_select().value() {
            0b001 => Device::AD16,
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

#[derive(Default, Debug, Clone, Copy)]
pub struct Channel {
    pub parameter: Parameter,
    pub control: Control,
    pub dma_base: Address,
    pub dma_length: u32,
    pub immediate: u32,
}

pub struct Interface {
    pub rtc: u32,
    pub sram: [u8; 64],
    pub waiting_trace_step: bool,
    pub sram_write_offset: u32,
    pub sram_write_remaining: u32,
    pub channels: [Channel; 3],
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            rtc: Default::default(),
            sram: [0; 64],
            waiting_trace_step: Default::default(),
            sram_write_offset: Default::default(),
            sram_write_remaining: Default::default(),
            channels: Default::default(),
        }
    }
}

impl System {
    fn exi_channel_0_transfer(&mut self) {
        let channel = self.external.channels[0];
        let device = channel.parameter.device0().unwrap();

        match device {
            Device::IplRtcSram => {
                tracing::debug!(pc = ?self.cpu.pc, control = ?channel.control, "IPL/RTC/SRAM write: 0x{:08X}", channel.immediate);

                if self.external.sram_write_remaining > 0 {
                    tracing::debug!(
                        "writing 0x{:08X} to SRAM 0x{:08X}",
                        channel.immediate,
                        self.external.sram_write_offset
                    );

                    let len = channel.control.imm_length();
                    channel.immediate.write_be_bytes(
                        &mut self.external.sram[self.external.sram_write_offset as usize..]
                            [..len as usize],
                    );

                    self.external.sram_write_offset += len;
                    self.external.sram_write_remaining -= len;

                    self.external.channels[0]
                        .control
                        .set_transfer_ongoing(false);

                    return;
                }

                match channel.immediate {
                    0x0000_0000..0x2000_0000 => {
                        if channel.control.dma() {
                            let ram_base = channel.dma_base.value() as usize;
                            let ipl_base = (channel.immediate >> 6) as usize;
                            let length = channel.dma_length as usize;

                            tracing::debug!(
                                pc = ?self.cpu.pc,
                                ram_base = ?Address(ram_base as u32),
                                ipl_base = ?Address(ipl_base as u32),
                                length,
                                "EXI DMA transfer from IPL ROM",
                            );

                            self.mem.ram[ram_base..][..length]
                                .copy_from_slice(&self.mem.ipl[ipl_base..][..length]);
                        } else {
                            tracing::debug!(
                                "set IPL base address to 0x{:08X} (0x{:08X})",
                                channel.immediate >> 6,
                                channel.immediate
                            );
                        }
                    }
                    0x2000_0000 => {
                        tracing::debug!("reading from RTC");
                        self.external.channels[0].immediate = self.external.rtc;
                    }
                    0xA000_0000 => {
                        tracing::debug!("writing to RTC");
                        self.external.rtc = self.external.channels[0].immediate;
                    }
                    0x2000_0100..0x2000_1100 => {
                        tracing::debug!("reading from SRAM");

                        if channel.control.dma() {
                            let ram_base = channel.dma_base.value() as usize;
                            let length = channel.dma_length as usize;

                            assert_eq!(length, 64);

                            tracing::debug!(
                                pc = ?self.cpu.pc,
                                ram_base = ?Address(ram_base as u32),
                                length,
                                "EXI DMA transfer from SRAM",
                            );

                            self.mem.ram[ram_base..][..length].copy_from_slice(&self.external.sram);
                        } else {
                            self.external.channels[0].immediate =
                                u32::read_be_bytes(&self.external.sram);
                        }
                    }
                    0xA000_0100..0xA000_1100 => {
                        let address = ((channel.immediate & 0xFFFF) - 0x0100) >> 6;
                        let remaining = 64 - address;
                        tracing::debug!(
                            "writing to SRAM address 0x{address:02X} - total bytes needed: {remaining}"
                        );

                        self.external.sram_write_offset = address;
                        self.external.sram_write_remaining = remaining;
                    }
                    _ => {
                        tracing::warn!("EXI channel 0 transfer ignored (SRAM?)");
                    }
                }
            }
            Device::SerialPort1 => {
                tracing::debug!(pc = ?self.cpu.pc, control = ?channel.control, "SP1 writing 0x{:08X}", channel.immediate);
            }
            _ => todo!("{:?}", device),
        }

        self.external.channels[0]
            .control
            .set_transfer_ongoing(false);
    }

    fn exi_channel_2_transfer(&mut self) {
        let channel = self.external.channels[2];
        let device = channel.parameter.device2().unwrap();

        match device {
            Device::AD16 => {
                assert!(!channel.control.dma());

                if self.external.waiting_trace_step {
                    tracing::info!("write to AD16 (trace step): 0x{:08X}", channel.immediate);
                    self.external.waiting_trace_step = false;
                } else {
                    match channel.immediate {
                        0x0000_0000 => self.external.channels[2].immediate = 0x04120000,
                        0xA000_0000 if !self.external.waiting_trace_step => {
                            self.external.waiting_trace_step = true;
                        }
                        _ => {
                            tracing::warn!("write to AD16 (unknown): 0x{:08X}", channel.immediate);
                        }
                    }
                }
            }
            _ => unreachable!(),
        }

        self.external.channels[2]
            .control
            .set_transfer_ongoing(false);
    }

    pub fn exi_update(&mut self) {
        if self.external.channels[0].control.transfer_ongoing() {
            self.exi_channel_0_transfer();
        }

        if self.external.channels[2].control.transfer_ongoing() {
            self.exi_channel_2_transfer();
        }
    }
}
