use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use image::{DynamicImage, Rgba, RgbaImage};

/// 512×512 transparent image with an opaque block off-centre.
fn icon_with_border() -> DynamicImage {
    let mut canvas = RgbaImage::new(512, 512);
    for y in 100..380 {
        for x in 60..300 {
            canvas.put_pixel(x, y, Rgba([200, 30, 30, 255]));
        }
    }
    DynamicImage::ImageRgba8(canvas)
}

fn crop_benchmark(c: &mut Criterion) {
    let bordered = icon_with_border();
    c.bench_function("crop 512x512 rgba8", |b| {
        b.iter_batched(
            || bordered.clone(),
            |image| spriters::crop(black_box(image), None),
            BatchSize::LargeInput,
        )
    });

    // Without alpha, the white border is cropped.
    let mut white = image::RgbImage::from_pixel(512, 512, image::Rgb([255, 255, 255]));
    for y in 100..380 {
        for x in 60..300 {
            white.put_pixel(x, y, image::Rgb([200, 30, 30]));
        }
    }
    let rgb = DynamicImage::ImageRgb8(white);
    c.bench_function("crop 512x512 rgb8 (white)", |b| {
        b.iter_batched(
            || rgb.clone(),
            |image| spriters::crop(black_box(image), None),
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, crop_benchmark);
criterion_main!(benches);
