mod mmio;

use crate::system::{
    Event, System, dsp,
    mem::{IPL_LEN, Memory, RAM_LEN},
    processor, video,
};
use common::{Address, Primitive};
use tracing::debug;
use zerocopy::IntoBytes;

pub use mmio::Mmio;

/// The bus of the system. Contains all memory mapped peripherals.
#[derive(Default)]
pub struct Bus {
    pub mem: Memory,
    pub dsp: dsp::Interface,
    pub video: video::Interface,
    pub processor: processor::Interface,
}

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
            0x0000_0000, RAM_LEN => Some(P::read_be_bytes(&self.bus.mem.ram.as_slice()[offset..])),
            0xFFF0_0000, IPL_LEN / 2 => Some(P::read_be_bytes(&self.bus.mem.ipl.as_slice()[offset..])),
            @default => None
        }
    }

    fn read_mmio<P: Primitive>(&mut self, offset: u16) -> P {
        let Some((reg, offset)) = Mmio::find(offset) else {
            tracing::warn!("reading from unknown mmio register ({offset:04X})");
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
            // === Video Interface ===
            Mmio::VideoVerticalTiming => ne!(self.bus.video.vertical_timing.as_bytes()),
            Mmio::VideoDisplayConfig => ne!(self.bus.video.display_config.as_bytes()),
            Mmio::VideoHorizontalTiming => ne!(self.bus.video.horizontal_timing.as_bytes()),
            Mmio::VideoOddVerticalTiming => ne!(self.bus.video.odd_vertical_timing.as_bytes()),
            Mmio::VideoEvenVerticalTiming => {
                ne!(self.bus.video.even_vertical_timing.as_bytes())
            }
            Mmio::VideoTopBaseLeft => ne!(self.bus.video.top_base_left.as_bytes()),
            Mmio::VideoTopBaseRight => ne!(self.bus.video.top_base_right.as_bytes()),
            Mmio::VideoBottomBaseLeft => ne!(self.bus.video.bottom_base_left.as_bytes()),
            Mmio::VideoBottomBaseRight => ne!(self.bus.video.bottom_base_right.as_bytes()),

            // Interrupts
            Mmio::VideoDisplayInterrupt0 => ne!(self.bus.video.interrupts[0].as_bytes()),
            Mmio::VideoDisplayInterrupt1 => ne!(self.bus.video.interrupts[1].as_bytes()),
            Mmio::VideoDisplayInterrupt2 => ne!(self.bus.video.interrupts[2].as_bytes()),
            Mmio::VideoDisplayInterrupt3 => ne!(self.bus.video.interrupts[3].as_bytes()),

            Mmio::VideoExternalFramebufferWidth => ne!(self.bus.video.xfb_width.as_bytes()),
            Mmio::VideoHorizontalScaling => ne!(self.bus.video.horizontal_scaling.as_bytes()),

            // Filter Coefficient Table
            Mmio::VideoFilterCoeff0
            | Mmio::VideoFilterCoeff1
            | Mmio::VideoFilterCoeff2
            | Mmio::VideoFilterCoeff3
            | Mmio::VideoFilterCoeff4
            | Mmio::VideoFilterCoeff5
            | Mmio::VideoFilterCoeff6 => P::default(), // NOTE: stubbed

            Mmio::VideoClock => ne!(self.bus.video.clock.as_bytes()),

            // === Processor Interface ===
            Mmio::ProcessorInterruptCause => ne!(self.bus.processor.cause.as_bytes()),
            Mmio::ProcessorInterruptMask => {
                ne!(self.bus.processor.mask.as_bytes())
            }

            // === DSP Interface ===
            Mmio::DspDspMailbox => ne!(self.bus.dsp.dsp_mailbox.as_bytes()),
            Mmio::DspCpuMailbox => {
                let data = ne!(self.bus.dsp.cpu_mailbox.as_bytes());
                if (2..4).contains(&offset) && self.bus.dsp.cpu_mailbox.status() {
                    debug!("clearing CPU mailbox");
                    self.bus.dsp.cpu_mailbox.set_status(false);
                }

                data
            }
            Mmio::DspControl => ne!(self.bus.dsp.control.as_bytes()),
            Mmio::DspAramDmaRamBase => ne!(self.bus.dsp.aram_dma_ram.as_bytes()),
            Mmio::DspAramDmaAramBase => ne!(self.bus.dsp.aram_dma_aram.as_bytes()),
            Mmio::DspAramDmaControl => ne!(self.bus.dsp.aram_dma_control.as_bytes()),
            _ => {
                tracing::warn!("unimplemented read from known mmio register ({reg:?})");
                P::default()
            }
        };

        tracing::debug!(
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
            0x0000_0000, RAM_LEN => P::read_be_bytes(&self.bus.mem.ram[offset..]),
            0xFFF0_0000, IPL_LEN / 2 => P::read_be_bytes(&self.bus.mem.ipl[offset..]),
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
            tracing::warn!("writing 0x{value:08X} to unknown mmio register ({offset:04X})");
            return;
        };

        // convert the range to native endian
        let range = if cfg!(target_endian = "big") {
            offset..offset + size_of::<P>()
        } else {
            let size = reg.size();
            (size as usize - offset - size_of::<P>())..(size as usize - offset)
        };

        tracing::debug!("writing 0x{:08X} to {:?}[{:?}]", value, reg, range,);

        // write to native endian bytes
        macro_rules! ne {
            ($bytes:expr) => {
                value.write_ne_bytes(&mut $bytes[range])
            };
        }

        match reg {
            // === Video Interface ===
            Mmio::VideoVerticalTiming => ne!(self.bus.video.vertical_timing.as_mut_bytes()),
            Mmio::VideoDisplayConfig => {
                ne!(self.bus.video.display_config.as_mut_bytes());
                self.update_video_interface();
            }
            Mmio::VideoHorizontalTiming => {
                ne!(self.bus.video.horizontal_timing.as_mut_bytes())
            }
            Mmio::VideoOddVerticalTiming => {
                ne!(self.bus.video.odd_vertical_timing.as_mut_bytes())
            }
            Mmio::VideoEvenVerticalTiming => {
                ne!(self.bus.video.even_vertical_timing.as_mut_bytes())
            }
            Mmio::VideoTopBaseLeft => ne!(self.bus.video.top_base_left.as_mut_bytes()),
            Mmio::VideoTopBaseRight => ne!(self.bus.video.top_base_right.as_mut_bytes()),
            Mmio::VideoBottomBaseLeft => ne!(self.bus.video.bottom_base_left.as_mut_bytes()),
            Mmio::VideoBottomBaseRight => ne!(self.bus.video.bottom_base_right.as_mut_bytes()),

            // Interrupts
            Mmio::VideoDisplayInterrupt0 => {
                let mut written = self.bus.video.interrupts[0];
                ne!(written.as_mut_bytes());
                self.bus.video.write_interrupt::<0>(written);
            }
            Mmio::VideoDisplayInterrupt1 => {
                let mut written = self.bus.video.interrupts[1];
                ne!(written.as_mut_bytes());
                self.bus.video.write_interrupt::<1>(written);
            }
            Mmio::VideoDisplayInterrupt2 => {
                let mut written = self.bus.video.interrupts[2];
                ne!(written.as_mut_bytes());
                self.bus.video.write_interrupt::<2>(written);
            }
            Mmio::VideoDisplayInterrupt3 => {
                let mut written = self.bus.video.interrupts[3];
                ne!(written.as_mut_bytes());
                self.bus.video.write_interrupt::<3>(written);
            }

            Mmio::VideoExternalFramebufferWidth => {
                ne!(self.bus.video.xfb_width.as_mut_bytes())
            }
            Mmio::VideoHorizontalScaling => {
                ne!(self.bus.video.horizontal_scaling.as_mut_bytes())
            }

            // Filter Coefficient Table
            Mmio::VideoFilterCoeff0
            | Mmio::VideoFilterCoeff1
            | Mmio::VideoFilterCoeff2
            | Mmio::VideoFilterCoeff3
            | Mmio::VideoFilterCoeff4
            | Mmio::VideoFilterCoeff5
            | Mmio::VideoFilterCoeff6 => (), // NOTE: stubbed

            Mmio::VideoClock => ne!(self.bus.video.clock.as_mut_bytes()),

            // === Processor Interface ===
            Mmio::ProcessorInterruptCause => {
                let mut written = self.bus.processor.cause;
                ne!(written.as_mut_bytes());
                self.bus.processor.write_cause(written);
            }
            Mmio::ProcessorInterruptMask => {
                self.scheduler.schedule(Event::CheckInterrupts, 0);
                ne!(self.bus.processor.mask.as_mut_bytes())
            }

            // === DSP Interface ===
            Mmio::DspDspMailbox => (),
            Mmio::DspCpuMailbox => (),
            Mmio::DspControl => {
                let mut written = self.bus.dsp.control;
                ne!(written.as_mut_bytes());
                self.bus.dsp.write_control(written);
            }
            Mmio::DspAramDmaRamBase => ne!(self.bus.dsp.aram_dma_ram.as_mut_bytes()),
            Mmio::DspAramDmaAramBase => ne!(self.bus.dsp.aram_dma_aram.as_mut_bytes()),
            Mmio::DspAramDmaControl => {
                debug!("started DSP DMA, set CPU mailbox as having data");

                // NOTE: stubbed, just set DMA as complete
                self.bus.dsp.control.set_aram_interrupt(true);

                // HACK: stub hack, set mailbox as having data
                self.bus.dsp.cpu_mailbox.set_status(true);
            }
            _ => tracing::warn!("unimplemented write to known mmio register ({reg:?})"),
        }
    }

    /// Writes a primitive to the given physical address.
    pub fn write<P: Primitive>(&mut self, addr: Address, value: P) {
        let offset: usize;
        map! {
            offset, addr;
            0x0000_0000, RAM_LEN => value.write_be_bytes(&mut self.bus.mem.ram[offset..]),
            0xFFF0_0000, IPL_LEN / 2 => value.write_be_bytes(&mut self.bus.mem.ipl[offset..]),
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
