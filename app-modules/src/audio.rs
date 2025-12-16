use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use hemisphere::{
    modules::audio::AudioModule,
    system::ai::{Frame, SampleRate},
};
use resampler::ResamplerFir;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use zerocopy::{FromBytes, Immutable, IntoBytes};

#[derive(Debug, Clone, Copy, Default, FromBytes, IntoBytes, Immutable)]
struct FrameF32 {
    left: f32,
    right: f32,
}

impl From<Frame> for FrameF32 {
    fn from(value: Frame) -> Self {
        Self {
            left: value.left as f32 / 32_768.0,
            right: value.right as f32 / 32_768.0,
        }
    }
}

struct State {
    sample_rate: SampleRate,
    resampler: ResamplerFir,
    resampled: Vec<f32>,
    frames: VecDeque<FrameF32>,
    last: FrameF32,
    writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
}

impl Drop for State {
    fn drop(&mut self) {
        self.writer.take().unwrap().finalize().unwrap();
    }
}

fn fill_buffer(state: &Arc<Mutex<State>>, out: &mut [f32]) {
    let mut state = state.lock().unwrap();
    let state = &mut *state;

    match state.sample_rate {
        SampleRate::KHz48 => {
            let mut last = state.last;
            for out in out.chunks_exact_mut(2) {
                let frame = if let Some(frame) = state.frames.pop_front() {
                    state
                        .writer
                        .as_mut()
                        .unwrap()
                        .write_sample(frame.left)
                        .unwrap();
                    state
                        .writer
                        .as_mut()
                        .unwrap()
                        .write_sample(frame.right)
                        .unwrap();

                    frame
                } else {
                    last
                };

                out[0] = frame.left;
                out[1] = frame.right;
                last = frame;
            }

            state.last = last;
        }
        SampleRate::KHz32 => {
            let slices = state.frames.as_slices();
            let frames = match (slices.0.is_empty(), slices.1.is_empty()) {
                (true, true) => slices.0,
                (false, true) => slices.0,
                (true, false) => slices.1,
                (false, false) => state.frames.make_contiguous(),
            };

            let samples: &[f32] = zerocopy::transmute_ref!(frames);
            let samples_needed = (2 * out.len()) / 3;

            let (consumed, produced) = state
                .resampler
                .resample(
                    &samples[..samples_needed.min(samples.len())],
                    &mut state.resampled,
                )
                .unwrap();

            state.frames.drain(..consumed / 2);

            let mut produced = state
                .resampled
                .chunks_exact(2)
                .map(|s| FrameF32 {
                    left: s[0],
                    right: s[1],
                })
                .take(produced / 2);

            let mut last = state.last;
            for out in out.chunks_exact_mut(2) {
                let frame = if let Some(frame) = produced.next() {
                    state
                        .writer
                        .as_mut()
                        .unwrap()
                        .write_sample(frame.left)
                        .unwrap();
                    state
                        .writer
                        .as_mut()
                        .unwrap()
                        .write_sample(frame.right)
                        .unwrap();

                    frame
                } else {
                    last
                };

                out[0] = frame.left;
                out[1] = frame.right;
                last = frame;
            }

            state.last = last;
        }
    }
}

pub struct CpalAudio {
    state: Arc<Mutex<State>>,
    _stream: Stream,
}

impl CpalAudio {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");

        let mut supported_configs = device
            .supported_output_configs()
            .expect("error while querying configs");

        let sample_rate = cpal::SampleRate(48_000);
        let config = supported_configs
            .find(|c| {
                c.sample_format() == cpal::SampleFormat::F32
                    && c.channels() == 2
                    && c.min_sample_rate() <= sample_rate
                    && c.max_sample_rate() >= sample_rate
            })
            .expect("no supported audio config")
            .with_sample_rate(sample_rate);

        let resampler = ResamplerFir::new(
            2,
            resampler::SampleRate::Hz32000,
            resampler::SampleRate::Hz48000,
            resampler::Latency::Sample64,
            resampler::Attenuation::Db90,
        );

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = hound::WavWriter::create("audio.wav", spec).unwrap();

        let state = State {
            sample_rate: SampleRate::KHz48,
            resampled: vec![0.0; resampler.buffer_size_output()],
            resampler,
            frames: VecDeque::with_capacity(8192),
            last: FrameF32::default(),
            writer: Some(writer),
        };

        let state = Arc::new(Mutex::new(state));
        let stream = device
            .build_output_stream(
                &config.into(),
                {
                    let state = state.clone();
                    move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        fill_buffer(&state, out);
                    }
                },
                move |_| panic!("audio errored"),
                None,
            )
            .unwrap();

        stream.play().unwrap();

        Self {
            state,
            _stream: stream,
        }
    }
}

impl AudioModule for CpalAudio {
    fn set_sample_rate(&mut self, sample_rate: SampleRate) {
        self.state.lock().unwrap().sample_rate = sample_rate;
    }

    fn play(&mut self, sample: Frame) {
        self.state.lock().unwrap().frames.push_back(sample.into());
    }
}
