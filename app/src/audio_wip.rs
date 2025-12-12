use cpal::{
    Sample as _,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use hemisphere::system::ai::Sample;
use std::{sync::mpsc::Receiver, time::Duration};

pub fn worker(sender: Receiver<Sample>) {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("no output device available");

    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");

    let sample_rate = cpal::SampleRate(48_042);
    let supported_config = supported_configs_range
        .find(|c| {
            c.sample_format() == cpal::SampleFormat::F32
                && c.channels() == 2
                && c.min_sample_rate() <= sample_rate
                && c.max_sample_rate() >= sample_rate
        })
        .expect("no supported config?!")
        .with_sample_rate(sample_rate);

    let stream = device
        .build_output_stream(
            &supported_config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for data in data.chunks_exact_mut(2) {
                    let Ok(pair) = sender.try_recv() else {
                        data[0] = f32::EQUILIBRIUM;
                        data[1] = f32::EQUILIBRIUM;
                        continue;
                    };

                    data[0] = pair.left.to_sample::<f32>();
                    data[1] = pair.right.to_sample::<f32>();
                }
            },
            move |_| panic!("errored :("),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    std::thread::sleep(Duration::from_secs(999));
}
