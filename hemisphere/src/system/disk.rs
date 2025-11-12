use crate::system::System;
use bitos::bitos;
use gekko::Address;
use std::io::SeekFrom;

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Status {
    #[bits(0)]
    pub break_request: bool,
    #[bits(1)]
    pub device_err_interrupt_mask: bool,
    #[bits(2)]
    pub device_err_interrupt: bool,
    #[bits(3)]
    pub transfer_interrupt_mask: bool,
    #[bits(4)]
    pub transfer_interrupt: bool,
    #[bits(5)]
    pub break_interrupt_mask: bool,
    #[bits(6)]
    pub break_interrupt: bool,
}

impl Status {
    pub fn any_interrupt(&self) -> bool {
        let device_err = self.device_err_interrupt() && self.device_err_interrupt();
        let transfer = self.transfer_interrupt() && self.transfer_interrupt_mask();
        let break_ = self.break_interrupt() && self.break_interrupt_mask();
        device_err || transfer || break_
    }
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    Read = 0,
    Write = 1,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Control {
    #[bits(0)]
    pub transfer_ongoing: bool,
    #[bits(1)]
    pub dma: bool,
    #[bits(2)]
    pub mode: TransferMode,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Cover {
    #[bits(0)]
    pub open: bool,
    #[bits(1)]
    pub interrupt_mask: bool,
    #[bits(2)]
    pub interrupt: bool,
}

#[derive(Default)]
pub struct Interface {
    pub status: Status,
    pub control: Control,
    pub command: [u32; 3],
    pub dma_base: Address,
    pub dma_length: u32,
    pub cover: Cover,
    pub config: u32,
}

impl Interface {
    pub fn write_status(&mut self, value: Status) {
        self.status
            .set_device_err_interrupt_mask(value.device_err_interrupt_mask());
        self.status.set_device_err_interrupt(
            self.status.device_err_interrupt() & !value.device_err_interrupt(),
        );

        self.status
            .set_transfer_interrupt_mask(value.transfer_interrupt_mask());
        self.status
            .set_transfer_interrupt(self.status.transfer_interrupt() & !value.transfer_interrupt());

        self.status
            .set_break_interrupt_mask(value.break_interrupt_mask());
        self.status
            .set_break_interrupt(self.status.break_interrupt() & !value.break_interrupt());
    }

    pub fn write_cover(&mut self, value: Cover) {
        self.cover.set_interrupt_mask(value.interrupt_mask());
        self.cover
            .set_interrupt(self.cover.interrupt() & !value.interrupt());
    }
}

impl System {
    pub fn di_complete_transfer(&mut self) {
        tracing::debug!("completed DI transfer");
        self.disk.status.set_transfer_interrupt(true);
        self.disk.control.set_transfer_ongoing(false);
        self.disk.dma_length = 0;
        self.pi_check_interrupts();
    }

    pub fn di_complete_seek(&mut self) {
        tracing::debug!("completed DI seek");
        self.disk.status.set_transfer_interrupt(true);
        self.disk.control.set_transfer_ongoing(false);
        self.pi_check_interrupts();
    }

    pub fn di_write_control(&mut self, value: Control) {
        self.disk.control.set_dma(value.dma());
        self.disk.control.set_mode(value.mode());

        if value.transfer_ongoing() && !self.disk.control.transfer_ongoing() {
            tracing::debug!("starting DI transfer");
            self.disk.control.set_transfer_ongoing(true);

            let command = self.disk.command[0];
            match command {
                0xA800_0000 => {
                    assert!(self.disk.control.dma());

                    // load from disk!
                    let target = self.disk.dma_base;
                    let offset = self.disk.command[1] << 2;
                    let length = self.disk.dma_length;

                    if length == 0 {
                        tracing::warn!(
                            "ignoring zero sized disk read from 0x{offset:08X} into {target}"
                        );
                        self.disk.control.set_transfer_ongoing(false);
                        return;
                    }

                    tracing::debug!(
                        "reading 0x{length:08X} bytes from disk at 0x{offset:08X} into {target}"
                    );

                    let iso = self.config.iso.as_mut().unwrap();
                    let reader = iso.reader();

                    let target = self.mmu.translate_instr_addr(target).unwrap();
                    reader.seek(SeekFrom::Start(offset as u64)).unwrap();
                    reader
                        .read_exact(&mut self.mem.ram[target.value() as usize..][..length as usize])
                        .unwrap();

                    self.scheduler.schedule(10000, System::di_complete_transfer);
                }
                0xA800_0040 => {
                    assert!(self.disk.control.dma());

                    // load from disk!
                    let target = self.disk.dma_base;
                    let offset = 0;
                    let length = self.disk.dma_length;

                    if length == 0 {
                        tracing::warn!(
                            "ignoring zero sized disk read from 0x{offset:08X} into {target}"
                        );
                        self.disk.control.set_transfer_ongoing(false);
                        return;
                    }

                    tracing::debug!(
                        "reading 0x{length:08X} bytes from disk at 0x{offset:08X} into {target}"
                    );

                    let iso = self.config.iso.as_mut().unwrap();
                    let reader = iso.reader();

                    let target = self.mmu.translate_instr_addr(target).unwrap();
                    reader.seek(SeekFrom::Start(offset as u64)).unwrap();
                    reader
                        .read_exact(&mut self.mem.ram[target.value() as usize..][..length as usize])
                        .unwrap();

                    self.scheduler.schedule(10000, System::di_complete_transfer);
                }
                0xAB00_0000 => {
                    tracing::warn!("doing disk seek! current implementation is half assed");
                    self.scheduler.schedule(10000, System::di_complete_seek);
                }
                0xE100_0000 | 0xE101_0000 => {
                    tracing::warn!("DISK HACK");
                    self.scheduler.schedule(10000, System::di_complete_seek);
                }
                0x1200_0000 => {
                    let target = self.mmu.translate_data_addr(self.disk.dma_base).unwrap();
                    let length = self.disk.dma_length;
                    self.mem.ram[target.value() as usize..][..length as usize].fill(0);

                    self.scheduler.schedule(10000, System::di_complete_transfer);
                }
                _ => todo!("{:08X}", command),
            }
        }
    }
}
