use cpal::{
    Sample as _, Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use hemisphere::{
    modules::audio::AudioModule,
    system::ai::{Sample, SampleRate},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct State {
    sample_rate: SampleRate,
    samples: VecDeque<Sample>,
}

fn fill_buffer(state: &Arc<Mutex<State>>, out: &mut [f32]) {
    let mut state = state.lock().unwrap();
    match state.sample_rate {
        SampleRate::KHz48 => {
            let mut last = Sample::default();
            for out in out.chunks_exact_mut(2) {
                let sample = state.samples.pop_front().unwrap_or(last);
                out[0] = sample.left.to_sample();
                out[1] = sample.left.to_sample();
                last = sample;
            }
        }
        SampleRate::KHz32 => {
            let mut index = 0f32;
            let mut last = Sample::default();
            for out in out.chunks_exact_mut(2) {
                let sample = if index > 1.0 {
                    index = index.fract();
                    state.samples.pop_front().unwrap_or(last)
                } else {
                    last
                };
                out[0] = sample.left.to_sample();
                out[1] = sample.left.to_sample();
                last = sample;

                index += 32.0 / 48.0;
            }
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

        let sample_rate = cpal::SampleRate(48_042);
        let config = supported_configs
            .find(|c| {
                c.sample_format() == cpal::SampleFormat::F32
                    && c.channels() == 2
                    && c.min_sample_rate() <= sample_rate
                    && c.max_sample_rate() >= sample_rate
            })
            .expect("no supported audio config")
            .with_sample_rate(sample_rate);

        let state = State {
            sample_rate: SampleRate::KHz48,
            samples: VecDeque::with_capacity(8192),
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

    fn play(&mut self, sample: Sample) {
        self.state.lock().unwrap().samples.push_back(sample);
    }
}
