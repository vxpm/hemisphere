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
    pub interrupt_enabled: bool,
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
    pub length_by_32: u15,
    #[bits(15)]
    pub transfer_ongoing: bool,
}

#[derive(Default)]
pub struct Interface {
    pub control: Control,
    pub dma_base: Address,
    pub dma_control: DmaControl,
    pub current_dma_sample: u32,
    pub sample_counter: u32,
    pub interrupt_sample: u32,
}

impl Interface {
    pub fn write_control(&mut self, value: Control) {
        self.control.set_playing(value.playing());
        self.control.set_aux_sample_rate(value.aux_sample_rate());
        self.control
            .set_interrupt_enabled(value.interrupt_enabled());
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

const SAMPLE_RATE: u32 = 48_000;
const CYCLES_PER_SAMPLE: u64 = gekko::FREQUENCY / SAMPLE_RATE as u64;

fn push_sample(sys: &mut System) {
    if sys.audio.dma_control.transfer_ongoing() {
        let addr = sys.audio.dma_base + 4 * sys.audio.current_dma_sample;
        let sample = sys.read::<u32>(addr);

        dbg!(sample);

        sys.audio.current_dma_sample += 1;

        let total_samples = sys.audio.dma_control.length_by_32().value() as u32 / 4;
        if sys.audio.current_dma_sample >= total_samples {
            println!("raising dma int");
            sys.dsp.control.set_ai_interrupt(true);
            sys.audio.current_dma_sample = 0;
            pi::check_interrupts(sys);
        }
    }

    println!("pushing sample {}", sys.audio.sample_counter);
    sys.audio.sample_counter += 1;

    if sys.audio.control.interrupt_valid() && sys.audio.sample_counter == sys.audio.interrupt_sample
    {
        println!("raising sample counter int");
        sys.audio.control.set_interrupt(true);
        pi::check_interrupts(sys);
    }

    sys.scheduler.schedule(CYCLES_PER_SAMPLE, self::push_sample);
}

pub fn start_playing(sys: &mut System) {
    if !sys.scheduler.contains(self::push_sample) {
        sys.scheduler.schedule(CYCLES_PER_SAMPLE, self::push_sample);
    }
}

pub fn stop_playing(sys: &mut System) {
    sys.scheduler.cancel(self::push_sample);
}
