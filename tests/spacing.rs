//! Padding is part of each image's CSS box, margin separates boxes (review 3.10).

use spriters::{Algorithm, Config, Manager};
use std::path::{Path, PathBuf};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_spacing_{}_{}",
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
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// `<tmp>/icons` with `a` (10×50, red) and `b` (30×10, blue); ordered a then b by height.
fn icons(tmp: &Path) -> (PathBuf, PathBuf) {
    let source = tmp.join("icons");
    let out = tmp.join("out");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    image::RgbaImage::from_pixel(10, 50, image::Rgba(RED))
        .save(source.join("a.png"))
        .unwrap();
    image::RgbaImage::from_pixel(30, 10, image::Rgba(BLUE))
        .save(source.join("b.png"))
        .unwrap();
    (source, out)
}

fn run(source: &Path, out: &Path, padding: Option<&str>, margin: Option<&str>) -> Manager {
    let config = Config {
        source: source.to_path_buf(),
        output: out.to_path_buf(),
        algorithm: Algorithm::Vertical,
        ordering: spriters::Ordering::Height,
        padding: padding.map(String::from),
        margin: margin.map(String::from),
        force: true,
        ..Default::default()
    };
    let mut manager = Manager {
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
    manager
}

fn sprite_png(out: &Path) -> image::RgbaImage {
    image::open(out.join("icons.png")).unwrap().to_rgba8()
}

fn css(out: &Path) -> String {
    std::fs::read_to_string(out.join("icons.css")).unwrap()
}

#[test]
fn padding_offsets_pixels_and_is_part_of_the_css_box() {
    // Review 3.10: with `-p 10` in vertical, images were drawn at the cell's corner.
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let manager = run(&source, &out, Some("10"), None);
        let sprite = &manager.sprites[0];

        // Boxes: a = 30×70 at (0,0), b = 50×30 at (0,70).
        assert_eq!(sprite.dimensions(), (50, 100));
        let png = sprite_png(&out);
        assert_eq!(png.dimensions(), (50, 100));
        // Pixels start inside the padding.
        assert_eq!(png.get_pixel(10, 10).0, RED);
        assert_eq!(png.get_pixel(9, 10).0, TRANSPARENT);
        assert_eq!(png.get_pixel(10, 9).0, TRANSPARENT);
        assert_eq!(png.get_pixel(10, 80).0, BLUE);
        assert_eq!(png.get_pixel(10, 79).0, TRANSPARENT);
        // Padding at the bottom of the last image is kept.
        assert_eq!(png.get_pixel(10, 89).0, BLUE);
        assert_eq!(png.get_pixel(10, 90).0, TRANSPARENT);

        // CSS boxes include the padding ("included in dimensions").
        let css = css(&out);
        assert!(
            css.contains("background-position: 0 0;\n  width: 30px;\n  height: 70px;"),
            "{css}"
        );
        assert!(
            css.contains("background-position: 0 -70px;\n  width: 50px;\n  height: 30px;"),
            "{css}"
        );
    });
}

#[test]
fn margin_separates_boxes_and_is_not_part_of_them() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let manager = run(&source, &out, None, Some("5"));
        let sprite = &manager.sprites[0];

        // Boxes: a = 10×50 at (5,5), b = 30×10 at (5,65); last margin included.
        assert_eq!(sprite.dimensions(), (40, 80));
        let png = sprite_png(&out);
        assert_eq!(png.dimensions(), (40, 80));
        assert_eq!(png.get_pixel(5, 5).0, RED);
        assert_eq!(png.get_pixel(4, 5).0, TRANSPARENT);
        assert_eq!(png.get_pixel(5, 65).0, BLUE);
        assert_eq!(png.get_pixel(5, 64).0, TRANSPARENT);

        let css = css(&out);
        assert!(
            css.contains("background-position: -5px -5px;\n  width: 10px;\n  height: 50px;"),
            "{css}"
        );
        assert!(
            css.contains("background-position: -5px -65px;\n  width: 30px;\n  height: 10px;"),
            "{css}"
        );
    });
}

#[test]
fn padding_and_margin_combine() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        // padding: top 1, right 2, bottom 3, left 4 — margin 5.
        let manager = run(&source, &out, Some("1 2 3 4"), Some("5"));
        let sprite = &manager.sprites[0];
        let a = &sprite.images[0];
        assert_eq!(
            (a.x, a.y, a.computed_width, a.computed_height),
            (5, 5, 16, 54)
        );
        let b = &sprite.images[1];
        // a's cell is 5 + 54 + 5 = 64 tall: b's box starts after its own top margin.
        assert_eq!(
            (b.x, b.y, b.computed_width, b.computed_height),
            (5, 69, 36, 14)
        );
        assert_eq!(sprite.dimensions(), (46, 88));

        let png = sprite_png(&out);
        assert_eq!(png.get_pixel(5 + 4, 5 + 1).0, RED);
        assert_eq!(png.get_pixel(5 + 4, 69 + 1).0, BLUE);
        assert_eq!(png.get_pixel(5 + 3, 69 + 1).0, TRANSPARENT);
    });
}

#[test]
fn no_spacing_is_unchanged() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let manager = run(&source, &out, None, None);
        let sprite = &manager.sprites[0];
        assert_eq!(sprite.dimensions(), (30, 60));
        assert_eq!((sprite.images[1].x, sprite.images[1].y), (0, 50));
        let png = sprite_png(&out);
        assert_eq!(png.get_pixel(0, 0).0, RED);
        assert_eq!(png.get_pixel(0, 50).0, BLUE);
    });
}
