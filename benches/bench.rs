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

/// Discovery of one large SVG (rasterisation dominates).
fn svg_benchmark(c: &mut Criterion) {
    let source = std::env::temp_dir().join(format!("spriters_bench_svg_{}", std::process::id()));
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(
        source.join("big.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024">
  <defs><linearGradient id="g"><stop offset="0" stop-color="#f00" stop-opacity="0.2"/>
  <stop offset="1" stop-color="#00f"/></linearGradient></defs>
  <rect width="1024" height="1024" fill="url(#g)"/>
  <circle cx="512" cy="512" r="400" fill="#0a0" fill-opacity="0.5"/>
</svg>"##,
    )
    .unwrap();
    let config = spriters::Config {
        source: source.clone(),
        ..Default::default()
    };
    c.bench_function("discover svg 1024x1024", |b| {
        b.iter(|| spriters::Sprite::new(black_box(&config), source.clone()))
    });
    std::fs::remove_dir_all(&source).ok();
}

criterion_group!(benches, crop_benchmark, svg_benchmark);
criterion_main!(benches);
