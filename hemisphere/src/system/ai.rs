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
    pub playing: bool,
}

#[derive(Default)]
pub struct Interface {
    pub control: Control,
    pub dma_base: Address,
    pub dma_control: DmaControl,
    pub current_dma_block: u32,
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
const CYCLES_PER_BLOCK: u64 = 8 * CYCLES_PER_SAMPLE;

fn push_streaming_sample(sys: &mut System) {
    sys.audio.sample_counter += 1;
    if sys.audio.control.interrupt_valid() && sys.audio.sample_counter == sys.audio.interrupt_sample
    {
        println!("raising sample counter int");
        sys.audio.control.set_interrupt(true);
        pi::check_interrupts(sys);
    }

    sys.scheduler
        .schedule(CYCLES_PER_SAMPLE, self::push_streaming_sample);
}

pub fn start_streaming(sys: &mut System) {
    if !sys.scheduler.contains(self::push_streaming_sample) {
        sys.scheduler
            .schedule(CYCLES_PER_SAMPLE, self::push_streaming_sample);
    }
}

pub fn stop_streaming(sys: &mut System) {
    sys.scheduler.cancel(self::push_streaming_sample);
}

#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub left: i16,
    pub right: i16,
}

fn push_data_dma_block(sys: &mut System) {
    let addr = sys.audio.dma_base + 32 * sys.audio.current_dma_block;
    let samples: [Sample; 8] = std::array::from_fn(|i| Sample {
        left: sys.read::<i16>(addr + 4 * i as u32),
        right: sys.read::<i16>(addr + 4 * i as u32 + 2),
    });

    for sample in samples {
        sys.config.audio_sink.send(sample).unwrap();
    }

    sys.audio.current_dma_block += 1;

    let total_blocks = sys.audio.dma_control.length_by_32().value() as u32;
    if sys.audio.current_dma_block >= total_blocks {
        sys.dsp.control.set_ai_interrupt(true);
        sys.audio.current_dma_block = 0;
        pi::check_interrupts(sys);
    }

    if sys.audio.dma_control.playing() {
        sys.scheduler
            .schedule(CYCLES_PER_BLOCK, self::push_data_dma_block);
    }
}

pub fn start_data_dma(sys: &mut System) {
    sys.scheduler
        .schedule(CYCLES_PER_BLOCK, self::push_data_dma_block);
}

pub fn stop_data_dma(sys: &mut System) {
    sys.scheduler.cancel(self::push_data_dma_block);
}
