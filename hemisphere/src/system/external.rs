use crate::system::System;
use bitos::{
    bitos,
    integer::{u2, u3},
};
use common::Address;

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

#[derive(Default, Debug, Clone, Copy)]
pub struct Channel {
    pub parameter: Parameter,
    pub control: Control,
    pub dma_base: Address,
    pub dma_length: u32,
    pub immediate: u32,
}

#[derive(Default)]
pub struct Interface {
    pub channels: [Channel; 3],
}

impl System {
    fn exi_channel_0_transfer(&mut self) {
        let channel = self.external.channels[0];
        match channel.parameter.device0().unwrap() {
            Device::IplRtcSram => {
                let offset = (channel.immediate >> 6) as usize;
                tracing::debug!(pc=?self.cpu.pc, control = ?channel.control, "writing 0x{:08X}", channel.immediate);

                match offset {
                    0x0000_0000..0x0080_0000 => {
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
                            // TODO: this breaks
                            // todo!()
                        }
                    }
                    _ => {
                        tracing::warn!("EXI channel 0 transfer ignored (SRAM or RTC)");
                    }
                }
            }
            _ => todo!(),
        }

        self.external.channels[0]
            .control
            .set_transfer_ongoing(false);
    }

    pub fn exi_update(&mut self) {
        if self.external.channels[0].control.transfer_ongoing() {
            self.exi_channel_0_transfer();
        }

        // HACK: ignore transfer in channel 2
        self.external.channels[2]
            .control
            .set_transfer_ongoing(false);
    }
}
