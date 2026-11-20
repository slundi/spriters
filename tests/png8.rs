//! `--png8`: 8-bit palette PNG with transparency (review 3.12).

use spriters::{Algorithm, Config, Manager, SpriteFormat};
use std::path::{Path, PathBuf};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_png8_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(&tmp);
    std::fs::remove_dir_all(&tmp).ok();
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn dirs(tmp: &Path) -> (PathBuf, PathBuf) {
    let source = tmp.join("icons");
    let out = tmp.join("out");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    (source, out)
}

fn run(source: &Path, out: &Path, extra: impl FnOnce(&mut Config)) {
    let mut config = Config {
        source: source.to_path_buf(),
        output: out.to_path_buf(),
        algorithm: Algorithm::Vertical,
        ordering: spriters::Ordering::Height,
        padding: Some(String::from("2")),
        png8: true,
        force: true,
        ..Default::default()
    };
    extra(&mut config);
    let mut manager = Manager {
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
}

/// Color type, bit depth and presence of a `tRNS` chunk of a PNG file.
fn png_format(path: &Path) -> (png::ColorType, png::BitDepth, bool) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let reader = decoder.read_info().unwrap();
    let info = reader.info();
    (info.color_type, info.bit_depth, info.trns.is_some())
}

fn rgba(path: &Path) -> image::RgbaImage {
    image::open(path).unwrap().to_rgba8()
}

#[test]
fn png8_is_a_palette_image_keeping_colours_and_transparency() {
    // Review 3.12: --png8 produced 8-bit grayscale without alpha (transparent → black).
    with_tmp_dir(|tmp| {
        let (source, out) = dirs(tmp);
        image::RgbaImage::from_pixel(10, 20, image::Rgba(RED))
            .save(source.join("a.png"))
            .unwrap();
        image::RgbaImage::from_pixel(10, 10, image::Rgba(BLUE))
            .save(source.join("b.png"))
            .unwrap();
        run(&source, &out, |_| {});

        let sprite = out.join("icons.png");
        assert_eq!(
            png_format(&sprite),
            (png::ColorType::Indexed, png::BitDepth::Eight, true)
        );
        let pixels = rgba(&sprite);
        // Colours are kept exactly (3 colours: red, blue, transparent).
        assert_eq!(pixels.get_pixel(2, 2).0, RED);
        assert_eq!(pixels.get_pixel(2, 26).0, BLUE);
        // The padding stays transparent instead of turning black.
        assert_eq!(pixels.get_pixel(0, 0).0[3], 0);
        assert_eq!(pixels.get_pixel(1, 25).0[3], 0);
    });
}

#[test]
fn png8_quantises_many_colours() {
    with_tmp_dir(|tmp| {
        let (source, out) = dirs(tmp);
        let gradient = image::RgbaImage::from_fn(40, 40, |x, y| {
            image::Rgba([(x * 6) as u8, (y * 6) as u8, ((x + y) * 3) as u8, 255])
        });
        gradient.save(source.join("gradient.png")).unwrap();
        run(&source, &out, |_| {});

        let sprite = out.join("icons.png");
        assert_eq!(png_format(&sprite).0, png::ColorType::Indexed);
        let pixels = rgba(&sprite);
        // Transparent padding stays exactly transparent, the gradient stays opaque and close.
        assert_eq!(pixels.get_pixel(0, 0).0[3], 0);
        for (x, y, expected) in gradient.enumerate_pixels() {
            let actual = pixels.get_pixel(x + 2, y + 2).0;
            assert!(actual[3] >= 240, "({x},{y}) became transparent: {actual:?}");
            for channel in 0..3 {
                let diff = (actual[channel] as i32 - expected.0[channel] as i32).abs();
                assert!(diff <= 24, "({x},{y}) {:?} -> {actual:?}", expected.0);
            }
        }
    });
}

#[test]
fn png8_applies_to_every_ratio() {
    with_tmp_dir(|tmp| {
        let (source, out) = dirs(tmp);
        image::RgbaImage::from_pixel(20, 20, image::Rgba(RED))
            .save(source.join("a.png"))
            .unwrap();
        run(&source, &out, |config| {
            config.ratios = Some(String::from("2,1"))
        });

        for file in ["icons.png", "icons@2x.png"] {
            assert_eq!(
                png_format(&out.join(file)).0,
                png::ColorType::Indexed,
                "{file}"
            );
        }
    });
}

#[test]
fn png8_is_ignored_for_webp() {
    with_tmp_dir(|tmp| {
        let (source, out) = dirs(tmp);
        image::RgbaImage::from_pixel(10, 10, image::Rgba(RED))
            .save(source.join("a.png"))
            .unwrap();
        run(&source, &out, |config| {
            config.sprite_format = SpriteFormat::WebP
        });

        let pixels = rgba(&out.join("icons.webp"));
        assert_eq!(pixels.get_pixel(2, 2).0, RED);
        assert_eq!(pixels.get_pixel(0, 0).0[3], 0);
    });
}
