use crate::system::ai::{Frame, SampleRate};

/// Trait for audio modules.
pub trait AudioModule {
    fn set_sample_rate(&mut self, sample_rate: SampleRate);
    fn play(&mut self, frame: Frame);
}

/// An implementation of [`AudioModule`] which does nothing.
#[derive(Debug, Clone, Copy)]
pub struct NopAudioModule;

impl AudioModule for NopAudioModule {
    fn set_sample_rate(&mut self, _: SampleRate) {}
    fn play(&mut self, _: Frame) {}
}
