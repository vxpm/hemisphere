mod mmio;

use crate::system::{
    Event, System, audio, disk, external,
    mem::{IPL_LEN, RAM_LEN},
};
use common::{Address, Primitive};
use tracing::debug;
use zerocopy::IntoBytes;

pub use mmio::Mmio;

/// Allows the usage of const values in patterns. It's a neat trick!
struct ConstTrick<const N: u32>;
impl<const N: u32> ConstTrick<N> {
    const OUTPUT: u32 = N;
}

macro_rules! map {
    ($offset:ident, $match_addr:expr; $($addr:expr, $size:expr => $block:expr,)* @default => $default:expr $(,)?) => {
        match $match_addr.value() {
            $(
                $addr..=ConstTrick::<{ $addr + ($size - 1) }>::OUTPUT => {
                    #[allow(unused_assignments)]
                    {
                        $offset = ($match_addr.value() - $addr) as usize;
                    }
                    $block
                }
            )*
            _ => $default
        }
    };
}

// WARN: Do not change CPU state in the bus methods, specially if they change the PC! These are
// called from within the JIT.

impl System {
    /// Reads a primitive from the given physical address, but only if it can't possibly have a
    /// side effect.
    pub fn read_pure<P: Primitive>(&self, addr: Address) -> Option<P> {
        let offset: usize;
        map! {
            offset, addr;
            0x0000_0000, RAM_LEN => Some(P::read_be_bytes(&self.mem.ram.as_slice()[offset..])),
            0xFFF0_0000, IPL_LEN / 2 => Some(P::read_be_bytes(&self.mem.ipl.as_slice()[offset..])),
            @default => None
        }
    }

    fn read_mmio<P: Primitive>(&mut self, offset: u16) -> P {
        let Some((reg, offset)) = Mmio::find(offset) else {
            tracing::error!(pc = ?self.cpu.pc, "reading from unknown mmio register ({offset:04X})");
            return P::default();
        };

        // read from native endian bytes
        macro_rules! ne {
            ($bytes:expr) => {{
                let range = if cfg!(target_endian = "big") {
                    offset..offset + size_of::<P>()
                } else {
                    let size = reg.size();
                    (size as usize - offset - size_of::<P>())..(size as usize - offset)
                };

                P::read_ne_bytes(&$bytes[range])
            }};
        }

        let value = match reg {
            // === Command Processor ===
            Mmio::CpStatus => ne!(self.gpu.command.status.as_bytes()),
            Mmio::CpControl => ne!(self.gpu.command.control.as_bytes()),
            Mmio::CpClear => ne!(&[0, 0]),
            Mmio::CpFifoStartLow => ne!(self.gpu.command.fifo.start.as_bytes()[0..2]),
            Mmio::CpFifoStartHigh => ne!(self.gpu.command.fifo.start.as_bytes()[2..4]),
            Mmio::CpFifoEndLow => ne!(self.gpu.command.fifo.end.as_bytes()[0..2]),
            Mmio::CpFifoEndHigh => ne!(self.gpu.command.fifo.end.as_bytes()[2..4]),
            Mmio::CpHighWatermarkLow => ne!(self.gpu.command.fifo.high_mark.as_bytes()[0..2]),
            Mmio::CpHighWatermarkHigh => ne!(self.gpu.command.fifo.high_mark.as_bytes()[2..4]),
            Mmio::CpLowWatermarkLow => ne!(self.gpu.command.fifo.low_mark.as_bytes()[0..2]),
            Mmio::CpLowWatermarkHigh => ne!(self.gpu.command.fifo.low_mark.as_bytes()[2..4]),
            Mmio::CpFifoCountLow => ne!(self.gpu.command.fifo.count.as_bytes()[0..2]),
            Mmio::CpFifoCountHigh => ne!(self.gpu.command.fifo.count.as_bytes()[2..4]),
            Mmio::CpFifoWritePtrLow => ne!(self.gpu.command.fifo.write_ptr.as_bytes()[0..2]),
            Mmio::CpFifoWritePtrHigh => ne!(self.gpu.command.fifo.write_ptr.as_bytes()[2..4]),
            Mmio::CpFifoReadPtrLow => ne!(self.gpu.command.fifo.read_ptr.as_bytes()[0..2]),
            Mmio::CpFifoReadPtrHigh => ne!(self.gpu.command.fifo.read_ptr.as_bytes()[2..4]),

            // === Pixel Engine ===
            Mmio::PixelInterruptStatus => ne!(self.gpu.pixel.interrupt.as_bytes()),

            // === Video Interface ===
            Mmio::VideoVerticalTiming => ne!(self.video.vertical_timing.as_bytes()),
            Mmio::VideoDisplayConfig => ne!(self.video.display_config.as_bytes()),
            Mmio::VideoHorizontalTiming => ne!(self.video.horizontal_timing.as_bytes()),
            Mmio::VideoOddVerticalTiming => ne!(self.video.odd_vertical_timing.as_bytes()),
            Mmio::VideoEvenVerticalTiming => {
                ne!(self.video.even_vertical_timing.as_bytes())
            }
            Mmio::VideoTopBaseLeft => ne!(self.video.top_base_left.as_bytes()),
            Mmio::VideoTopBaseRight => ne!(self.video.top_base_right.as_bytes()),
            Mmio::VideoBottomBaseLeft => ne!(self.video.bottom_base_left.as_bytes()),
            Mmio::VideoBottomBaseRight => ne!(self.video.bottom_base_right.as_bytes()),

            // Interrupts
            Mmio::VideoDisplayInterrupt0 => ne!(self.video.interrupts[0].as_bytes()),
            Mmio::VideoDisplayInterrupt1 => ne!(self.video.interrupts[1].as_bytes()),
            Mmio::VideoDisplayInterrupt2 => ne!(self.video.interrupts[2].as_bytes()),
            Mmio::VideoDisplayInterrupt3 => ne!(self.video.interrupts[3].as_bytes()),

            Mmio::VideoExternalFramebufferWidth => ne!(self.video.xfb_width.as_bytes()),
            Mmio::VideoHorizontalScaling => ne!(self.video.horizontal_scaling.as_bytes()),

            // Filter Coefficient Table
            Mmio::VideoFilterCoeff0
            | Mmio::VideoFilterCoeff1
            | Mmio::VideoFilterCoeff2
            | Mmio::VideoFilterCoeff3
            | Mmio::VideoFilterCoeff4
            | Mmio::VideoFilterCoeff5
            | Mmio::VideoFilterCoeff6 => P::default(), // NOTE: stubbed

            Mmio::VideoClock => ne!(self.video.clock.as_bytes()),

            // === Processor Interface ===
            // Interrupts
            Mmio::ProcessorInterruptCause => {
                ne!((self.get_raised_interrupts().to_bits().value() as u32).as_bytes())
            }
            Mmio::ProcessorInterruptMask => ne!(self.processor.mask.as_bytes()),

            // FIFO
            Mmio::ProcessorFifoStart => ne!(self.processor.fifo_start.as_bytes()),
            Mmio::ProcessorFifoEnd => ne!(self.processor.fifo_end.as_bytes()),
            Mmio::ProcessorFifoCurrent => ne!(self.processor.fifo_current.as_bytes()),

            // === DSP Interface ===
            Mmio::DspDspMailbox => ne!(self.dsp.dsp_mailbox.as_bytes()),
            Mmio::DspCpuMailbox => {
                let data = ne!(self.dsp.cpu_mailbox.as_bytes());
                if (2..4).contains(&offset) && self.dsp.cpu_mailbox.status() {
                    debug!("popping CPU mailbox");
                    self.dsp.pop_cpu_mailbox();
                }

                data
            }
            Mmio::DspControl => ne!(self.dsp.control.as_bytes()),
            Mmio::DspAramMode => ne!((!0u64).as_mut_bytes()), // TODO: figure out this register
            Mmio::DspAramDmaRamBase => ne!(self.dsp.aram_dma_ram.as_bytes()),
            Mmio::DspAramDmaAramBase => ne!(self.dsp.aram_dma_aram.as_bytes()),
            Mmio::DspAramDmaControl => ne!(self.dsp.aram_dma_control.as_bytes()),

            // === Disk Interface ===
            Mmio::DiskStatus => ne!(self.disk.status.as_bytes()),
            Mmio::DiskCover => ne!(self.disk.cover.as_bytes()),
            Mmio::DiskDmaBase => ne!(self.disk.dma_base.as_bytes()),
            Mmio::DiskDmaLength => ne!(self.disk.dma_length.as_bytes()),
            Mmio::DiskControl => ne!(self.disk.control.as_bytes()),
            Mmio::DiskConfiguration => ne!(self.disk.config.as_bytes()),

            // === External Interface ===
            Mmio::ExiChannel0Param => ne!(self.external.channel0.parameter.as_bytes()),
            Mmio::ExiChannel0DmaBase => ne!(self.external.channel0.dma_base.as_bytes()),
            Mmio::ExiChannel0DmaLength => ne!(self.external.channel0.dma_length.as_bytes()),
            Mmio::ExiChannel0Control => ne!(self.external.channel0.control.as_bytes()),
            Mmio::ExiChannel0Immediate => ne!(self.external.channel0.immediate.as_bytes()),

            Mmio::ExiChannel1Param => ne!(self.external.channel1.parameter.as_bytes()),
            Mmio::ExiChannel1DmaBase => ne!(self.external.channel1.dma_base.as_bytes()),
            Mmio::ExiChannel1DmaLength => ne!(self.external.channel1.dma_length.as_bytes()),
            Mmio::ExiChannel1Control => ne!(self.external.channel1.control.as_bytes()),
            Mmio::ExiChannel1Immediate => ne!(self.external.channel1.immediate.as_bytes()),

            Mmio::ExiChannel2Param => ne!(self.external.channel2.parameter.as_bytes()),
            Mmio::ExiChannel2DmaBase => ne!(self.external.channel2.dma_base.as_bytes()),
            Mmio::ExiChannel2DmaLength => ne!(self.external.channel2.dma_length.as_bytes()),
            Mmio::ExiChannel2Control => ne!(self.external.channel2.control.as_bytes()),
            Mmio::ExiChannel2Immediate => ne!(self.external.channel2.immediate.as_bytes()),

            // === Audio Interface ===
            Mmio::AudioSampleCounter => {
                // HACK: allows IPL to progress further
                self.audio.sample_counter += 1;
                ne!(self.audio.sample_counter.as_bytes())
            }
            Mmio::AudioControl => ne!(self.audio.control.as_bytes()),

            _ => {
                tracing::warn!(pc = ?self.cpu.pc, "unimplemented read from known mmio register ({reg:?})");
                P::default()
            }
        };

        tracing::debug!(
            pc = ?self.cpu.pc,
            "reading from {:?}[{}..{}]: {:08X}",
            reg,
            offset,
            offset + size_of::<P>(),
            value
        );

        value
    }

    /// Reads a primitive from the given physical address.
    pub fn read<P: Primitive>(&mut self, addr: Address) -> P {
        let offset: usize;
        map! {
            offset, addr;
            0x0000_0000, RAM_LEN => P::read_be_bytes(&self.mem.ram[offset..]),
            0xFFF0_0000, IPL_LEN / 2 => P::read_be_bytes(&self.mem.ipl[offset..]),
            @default => {
                std::hint::cold_path();
                if addr.value() & 0xFFFF_0000 != 0x0C00_0000 {
                    std::hint::cold_path();
                    tracing::error!("reading from {addr} (unknown region)");
                    return P::default();
                }

                self.read_mmio(addr.value() as u16)
            },
        }
    }

    fn write_mmio<P: Primitive>(&mut self, offset: u16, value: P) {
        let Some((reg, offset)) = Mmio::find(offset) else {
            tracing::error!("writing 0x{value:08X} to unknown mmio register ({offset:04X})");
            return;
        };

        // convert the range to native endian
        let range = if cfg!(target_endian = "big") {
            offset..offset + size_of::<P>()
        } else {
            let size = reg.size();
            (size as usize - offset - size_of::<P>())..(size as usize - offset)
        };

        if !matches!(reg, Mmio::FakeStdout | Mmio::ProcessorFifo) {
            tracing::debug!("writing 0x{:08X} to {:?}[{:?}]", value, reg, range,);
        }

        // write to native endian bytes
        macro_rules! ne {
            ($bytes:expr) => {
                value.write_ne_bytes(&mut $bytes[range])
            };
        }

        match reg {
            // === Command Processor ===
            Mmio::CpStatus => ne!(self.gpu.command.status.as_mut_bytes()),
            Mmio::CpControl => ne!(self.gpu.command.control.as_mut_bytes()),
            Mmio::CpClear => {
                let mut written = 0;
                ne!(written.as_mut_bytes());
                self.gpu.command.write_clear(written);
            }
            Mmio::CpFifoStartLow => ne!(self.gpu.command.fifo.start.as_mut_bytes()[0..2]),
            Mmio::CpFifoStartHigh => ne!(self.gpu.command.fifo.start.as_mut_bytes()[2..4]),
            Mmio::CpFifoEndLow => ne!(self.gpu.command.fifo.end.as_mut_bytes()[0..2]),
            Mmio::CpFifoEndHigh => ne!(self.gpu.command.fifo.end.as_mut_bytes()[2..4]),
            Mmio::CpHighWatermarkLow => {
                ne!(self.gpu.command.fifo.high_mark.as_mut_bytes()[0..2])
            }
            Mmio::CpHighWatermarkHigh => {
                ne!(self.gpu.command.fifo.high_mark.as_mut_bytes()[2..4])
            }
            Mmio::CpLowWatermarkLow => {
                ne!(self.gpu.command.fifo.low_mark.as_mut_bytes()[0..2])
            }
            Mmio::CpLowWatermarkHigh => {
                ne!(self.gpu.command.fifo.low_mark.as_mut_bytes()[2..4])
            }
            Mmio::CpFifoCountLow => ne!(self.gpu.command.fifo.count.as_mut_bytes()[0..2]),
            Mmio::CpFifoCountHigh => ne!(self.gpu.command.fifo.count.as_mut_bytes()[2..4]),
            Mmio::CpFifoWritePtrLow => {
                ne!(self.gpu.command.fifo.write_ptr.as_mut_bytes()[0..2])
            }
            Mmio::CpFifoWritePtrHigh => {
                ne!(self.gpu.command.fifo.write_ptr.as_mut_bytes()[2..4])
            }
            Mmio::CpFifoReadPtrLow => ne!(self.gpu.command.fifo.read_ptr.as_mut_bytes()[0..2]),
            Mmio::CpFifoReadPtrHigh => ne!(self.gpu.command.fifo.read_ptr.as_mut_bytes()[2..4]),

            // === Pixel Engine ===
            Mmio::PixelInterruptStatus => {
                let mut written = 0;
                ne!(written.as_mut_bytes());
                self.gpu.pixel.write_interrupt(written);
            }

            // === Video Interface ===
            Mmio::VideoVerticalTiming => ne!(self.video.vertical_timing.as_mut_bytes()),
            Mmio::VideoDisplayConfig => {
                ne!(self.video.display_config.as_mut_bytes());
                self.update_video_interface();
            }
            Mmio::VideoHorizontalTiming => {
                ne!(self.video.horizontal_timing.as_mut_bytes())
            }
            Mmio::VideoOddVerticalTiming => {
                ne!(self.video.odd_vertical_timing.as_mut_bytes())
            }
            Mmio::VideoEvenVerticalTiming => {
                ne!(self.video.even_vertical_timing.as_mut_bytes())
            }
            Mmio::VideoTopBaseLeft => ne!(self.video.top_base_left.as_mut_bytes()),
            Mmio::VideoTopBaseRight => ne!(self.video.top_base_right.as_mut_bytes()),
            Mmio::VideoBottomBaseLeft => ne!(self.video.bottom_base_left.as_mut_bytes()),
            Mmio::VideoBottomBaseRight => ne!(self.video.bottom_base_right.as_mut_bytes()),

            // Interrupts
            Mmio::VideoDisplayInterrupt0 => {
                let mut written = self.video.interrupts[0];
                ne!(written.as_mut_bytes());
                self.video.write_interrupt::<0>(written);
            }
            Mmio::VideoDisplayInterrupt1 => {
                let mut written = self.video.interrupts[1];
                ne!(written.as_mut_bytes());
                self.video.write_interrupt::<1>(written);
            }
            Mmio::VideoDisplayInterrupt2 => {
                let mut written = self.video.interrupts[2];
                ne!(written.as_mut_bytes());
                self.video.write_interrupt::<2>(written);
            }
            Mmio::VideoDisplayInterrupt3 => {
                let mut written = self.video.interrupts[3];
                ne!(written.as_mut_bytes());
                self.video.write_interrupt::<3>(written);
            }

            Mmio::VideoExternalFramebufferWidth => {
                ne!(self.video.xfb_width.as_mut_bytes())
            }
            Mmio::VideoHorizontalScaling => {
                ne!(self.video.horizontal_scaling.as_mut_bytes())
            }

            // Filter Coefficient Table
            Mmio::VideoFilterCoeff0
            | Mmio::VideoFilterCoeff1
            | Mmio::VideoFilterCoeff2
            | Mmio::VideoFilterCoeff3
            | Mmio::VideoFilterCoeff4
            | Mmio::VideoFilterCoeff5
            | Mmio::VideoFilterCoeff6 => (), // NOTE: stubbed

            Mmio::VideoClock => ne!(self.video.clock.as_mut_bytes()),

            // === Processor Interface ===
            // Interrupts
            Mmio::ProcessorInterruptMask => {
                self.scheduler.schedule_now(Event::CheckInterrupts);
                ne!(self.processor.mask.as_mut_bytes())
            }

            // FIFO
            Mmio::ProcessorFifoStart => ne!(self.processor.fifo_start.as_mut_bytes()),
            Mmio::ProcessorFifoEnd => ne!(self.processor.fifo_end.as_mut_bytes()),
            Mmio::ProcessorFifoCurrent => ne!(self.processor.fifo_current.as_mut_bytes()),

            // === DSP Interface ===
            Mmio::DspDspMailbox => (),
            Mmio::DspCpuMailbox => (),
            Mmio::DspControl => {
                let mut written = self.dsp.control;
                ne!(written.as_mut_bytes());
                self.dsp.write_control(written);
            }
            Mmio::DspAramDmaRamBase => ne!(self.dsp.aram_dma_ram.as_mut_bytes()),
            Mmio::DspAramDmaAramBase => ne!(self.dsp.aram_dma_aram.as_mut_bytes()),
            Mmio::DspAramDmaControl => {
                debug!("started DSP DMA, set CPU mailbox as having data");

                // NOTE: stubbed, just set DMA as complete
                self.dsp.control.set_aram_interrupt(true);

                self.dsp.cpu_mailbox_queue.push_back(0);

                // HACK: stub hack, set mailbox as having data
                self.dsp.cpu_mailbox.set_status(true);
            }
            Mmio::AudioDmaControl => {
                debug!("forcing AI DMA");
                self.audio
                    .control
                    .set_interrupt(self.audio.control.interrupt_mask());
                self.scheduler.schedule_now(Event::CheckInterrupts);
            }

            // === Disk Interface ===
            Mmio::DiskStatus => {
                let mut written = disk::Status::from_bits(0);
                ne!(written.as_mut_bytes());
                self.disk.write_status(written);
                tracing::debug!(diskstatus = ?self.disk.status);
            }
            Mmio::DiskCover => {
                let mut written = disk::Cover::from_bits(0);
                ne!(written.as_mut_bytes());
                self.disk.write_cover(written);
                self.disk.cover.set_open(false);
                tracing::debug!(diskcover = ?self.disk.cover);
            }
            Mmio::DiskCommand0 => ne!(self.disk.command[0].as_mut_bytes()),
            Mmio::DiskCommand1 => ne!(self.disk.command[1].as_mut_bytes()),
            Mmio::DiskCommand2 => ne!(self.disk.command[2].as_mut_bytes()),
            Mmio::DiskDmaBase => ne!(self.disk.dma_base.as_mut_bytes()),
            Mmio::DiskDmaLength => ne!(self.disk.dma_length.as_mut_bytes()),
            Mmio::DiskControl => {
                ne!(self.disk.control.as_mut_bytes());
                self.di_update();
            }
            Mmio::DiskConfiguration => {
                ne!(self.disk.config.as_mut_bytes());
            }

            // === External Interface ===
            Mmio::ExiChannel0Param => {
                let mut written = external::Parameter::from_bits(0);
                ne!(written.as_mut_bytes());
                self.external.channel0.parameter.write(written);
            }
            Mmio::ExiChannel0DmaBase => ne!(self.external.channel0.dma_base.as_mut_bytes()),
            Mmio::ExiChannel0DmaLength => ne!(self.external.channel0.dma_length.as_mut_bytes()),
            Mmio::ExiChannel0Control => {
                ne!(self.external.channel0.control.as_mut_bytes());
                tracing::debug!("{:?}", self.external.channel0.control);
                self.exi_update();
            }
            Mmio::ExiChannel0Immediate => ne!(self.external.channel0.immediate.as_mut_bytes()),
            Mmio::ExiChannel1Param => {
                let mut written = external::Parameter::from_bits(0);
                ne!(written.as_mut_bytes());
                self.external.channel1.parameter.write(written);
            }
            Mmio::ExiChannel1DmaBase => ne!(self.external.channel1.dma_base.as_mut_bytes()),
            Mmio::ExiChannel1DmaLength => ne!(self.external.channel1.dma_length.as_mut_bytes()),
            Mmio::ExiChannel1Control => {
                ne!(self.external.channel1.control.as_mut_bytes());
                self.exi_update();
            }
            Mmio::ExiChannel1Immediate => ne!(self.external.channel1.immediate.as_mut_bytes()),
            Mmio::ExiChannel2Param => {
                let mut written = external::Parameter::from_bits(0);
                ne!(written.as_mut_bytes());
                self.external.channel2.parameter.write(written);
            }
            Mmio::ExiChannel2DmaBase => ne!(self.external.channel2.dma_base.as_mut_bytes()),
            Mmio::ExiChannel2DmaLength => ne!(self.external.channel2.dma_length.as_mut_bytes()),
            Mmio::ExiChannel2Control => {
                ne!(self.external.channel2.control.as_mut_bytes());
                self.exi_update();
            }
            Mmio::ExiChannel2Immediate => ne!(self.external.channel2.immediate.as_mut_bytes()),

            // === Audio Interface ===
            Mmio::AudioControl => {
                let mut written = audio::Control::from_bits(0);
                ne!(written.as_mut_bytes());
                self.audio.write_control(written);
            }

            // === Fake STDOUT ===
            Mmio::FakeStdout => {
                let mut written = 0u8;
                ne!(written.as_mut_bytes());
                print!("{}", written as char);
            }

            // === PI FIFO ===
            Mmio::ProcessorFifo => {
                let mut buf = [0; 4];
                value.write_be_bytes(&mut buf);

                let bytes = &buf[0..size_of::<P>()];
                for byte in bytes {
                    self.pi_fifo_push(*byte);
                }
            }
            _ => tracing::warn!("unimplemented write to known mmio register ({reg:?})"),
        }
    }

    /// Writes a primitive to the given physical address.
    pub fn write<P: Primitive>(&mut self, addr: Address, value: P) {
        let offset: usize;
        map! {
            offset, addr;
            0x0000_0000, RAM_LEN => value.write_be_bytes(&mut self.mem.ram[offset..]),
            0xFFF0_0000, IPL_LEN / 2 => value.write_be_bytes(&mut self.mem.ipl[offset..]),
            @default => {
                std::hint::cold_path();
                if addr.value() & 0xFFFF_0000 != 0x0C00_0000 {
                    std::hint::cold_path();
                    tracing::error!("writing 0x{value:08X} to {addr} (unknown region)");
                    return;
                }

                self.write_mmio(addr.value() as u16, value);
            },
        }
    }
}
