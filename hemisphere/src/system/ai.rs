//! Audio interface (AI).
use crate::system::{System, pi};
use bitos::{bitos, integer::u15};
use gekko::Address;

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleRate {
    KHz48 = 0,
    KHz32 = 1,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Control {
    #[bits(0)]
    pub playing: bool,
    #[bits(1)]
    pub aux_sample_rate: SampleRate,
    #[bits(2)]
    pub interrupt_mask: bool,
    #[bits(3)]
    pub interrupt: bool,
    #[bits(4)]
    pub interrupt_valid: bool,
    #[bits(5)]
    pub sample_counter_reset: bool,
    #[bits(6)]
    pub dsp_sample_rate: SampleRate,
}

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DmaControl {
    #[bits(0..15)]
    pub length: u15,
    #[bits(15)]
    pub transfer_ongoing: bool,
}

#[derive(Default)]
pub struct Interface {
    pub control: Control,
    pub dma_base: Address,
    pub dma_control: DmaControl,
    pub current_sample: u32,
    pub sample_counter: u32,
}

impl Interface {
    pub fn write_control(&mut self, value: Control) {
        self.control.set_playing(value.playing());
        self.control.set_aux_sample_rate(value.aux_sample_rate());
        self.control.set_interrupt_mask(value.interrupt_mask());
        self.control
            .set_interrupt(self.control.interrupt() & !value.interrupt());
        self.control.set_interrupt_valid(value.interrupt_valid());

        if value.sample_counter_reset() {
            self.sample_counter = 0;
        }

        self.control.set_dsp_sample_rate(value.dsp_sample_rate());
    }
}

// pub fn do_dma(sys: &mut System) {
//     tracing::debug!("AI DMA finished");
//     sys.dsp.control.set_ai_interrupt(true);
//     pi::check_interrupts(sys);
//
//     if sys.audio.dma_control.transfer_ongoing() {
//         sys.scheduler.schedule(1620000, do_dma);
//     }
// }

const SAMPLE_RATE: u32 = 32_000;
const CYCLES_PER_SAMPLE: u64 = gekko::FREQUENCY / SAMPLE_RATE as u64;

fn push_sample(sys: &mut System) {
    let addr = sys.audio.dma_base + 4 * sys.audio.current_sample;
    let sample = sys.read::<u32>(addr);

    dbg!(sample);

    sys.audio.current_sample += 1;
    sys.audio.sample_counter += 1;

    let total_samples = sys.audio.dma_control.length().value() as u32 / 4;
    if sys.audio.current_sample >= total_samples {
        sys.dsp.control.set_ai_interrupt(true);
        pi::check_interrupts(sys);
        sys.audio.current_sample = 0;
    }

    sys.scheduler.schedule(CYCLES_PER_SAMPLE, self::push_sample);
}

pub fn start_playing(sys: &mut System) {
    sys.scheduler.schedule(CYCLES_PER_SAMPLE, self::push_sample);
}

pub fn stop_playing(sys: &mut System) {
    sys.scheduler.cancel(self::push_sample);
}
