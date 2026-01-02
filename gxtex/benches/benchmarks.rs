use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gxtex::{Format, Pixel, Rgb565, SimdRgb565, compute_size};

fn rgb565(c: &mut Criterion) {
    let img = image::open("resources/waterfall.webp").unwrap();
    let pixels = img
        .to_rgba8()
        .pixels()
        .map(|p| Pixel {
            r: p.0[0],
            g: p.0[1],
            b: p.0[2],
            a: p.0[3],
        })
        .collect::<Vec<_>>();

    let required_width = (img.width() as usize).next_multiple_of(Rgb565::TILE_WIDTH);
    let required_height = (img.height() as usize).next_multiple_of(Rgb565::TILE_HEIGHT);
    let mut encoded = vec![0; compute_size::<Rgb565>(required_width, required_height)];
    gxtex::encode::<Rgb565>(
        &(),
        required_width / Rgb565::TILE_WIDTH,
        img.width() as usize,
        img.height() as usize,
        black_box(&pixels),
        &mut encoded,
    );

    let mut group = c.benchmark_group("RGB565");
    group.throughput(criterion::Throughput::Bytes(encoded.len() as u64));

    group.bench_function("Simple", |b| {
        b.iter_with_large_drop(|| {
            gxtex::decode::<Rgb565>(
                img.width() as usize,
                img.height() as usize,
                black_box(&encoded),
            )
        })
    });

    group.bench_function("SIMD", |b| {
        b.iter_with_large_drop(|| {
            gxtex::decode::<SimdRgb565>(
                img.width() as usize,
                img.height() as usize,
                black_box(&encoded),
            )
        })
    });

    group.finish();
}

criterion_group!(benches, rgb565);
criterion_main!(benches);
