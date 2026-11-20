//! `--ratios` / `--retina`: one image per ratio, CSS in 1× units (review 3.9).

use spriters::{Algorithm, Config, Manager};
use std::path::{Path, PathBuf};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_retina_{}_{}",
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

/// `<tmp>/icons` with two icons drawn at 2×: `a` (40×60 px) and `b` (40×40 px).
fn retina_icons(tmp: &Path) -> (PathBuf, PathBuf) {
    let source = tmp.join("icons");
    let out = tmp.join("out");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    image::RgbaImage::from_pixel(40, 60, image::Rgba(RED))
        .save(source.join("a.png"))
        .unwrap();
    image::RgbaImage::from_pixel(40, 40, image::Rgba(BLUE))
        .save(source.join("b.png"))
        .unwrap();
    (source, out)
}

fn config(source: &Path, out: &Path, ratios: &str) -> Config {
    Config {
        source: source.to_path_buf(),
        output: out.to_path_buf(),
        algorithm: Algorithm::Vertical,
        ratios: Some(ratios.to_owned()),
        force: true,
        ..Default::default()
    }
}

fn run(config: &Config) -> Manager {
    let mut manager = Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
    manager
}

fn dimensions(path: PathBuf) -> (u32, u32) {
    image::image_dimensions(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

fn pixel(path: PathBuf, x: u32, y: u32) -> [u8; 4] {
    image::open(path).unwrap().to_rgba8().get_pixel(x, y).0
}

#[test]
fn one_image_per_ratio() {
    // Review 3.9: a single image was generated, at the highest resolution.
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        let manager = run(&config(&source, &out, "2,1"));

        // 1× layout (CSS pixels): a = 20×30, b = 20×20, stacked vertically.
        assert_eq!(manager.sprites[0].dimensions(), (20, 50));
        assert_eq!(dimensions(out.join("icons.png")), (20, 50));
        assert_eq!(dimensions(out.join("icons@2x.png")), (40, 100));

        // Pixels land in the right place at both ratios.
        assert_eq!(pixel(out.join("icons@2x.png"), 5, 5), RED);
        assert_eq!(pixel(out.join("icons@2x.png"), 5, 70), BLUE);
        assert_eq!(pixel(out.join("icons.png"), 5, 5), RED);
        assert_eq!(pixel(out.join("icons.png"), 5, 40), BLUE);
    });
}

#[test]
fn css_uses_1x_units_and_switches_image_per_ratio() {
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        run(&config(&source, &out, "2,1"));
        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();

        // Base rule: 1× image, displayed at the 1× sprite size.
        assert!(css.contains("background-image: url('icons.png');"), "{css}");
        assert!(css.contains("background-size: 20px 50px;"), "{css}");
        // Positions and sizes in CSS pixels (not the 2× source pixels).
        assert!(css.contains("width: 20px;\n  height: 30px;"), "{css}");
        assert!(css.contains("background-position: 0 -30px;"), "{css}");
        assert!(!css.contains("width: 40px"), "{css}");
        // The 2× media query switches to the @2x image.
        let media = &css[css.find("min-resolution: 2dppx").expect("2x media query")..];
        assert!(
            media.contains("background-image: url('icons@2x.png');"),
            "{css}"
        );
        assert!(
            !css.contains("1dppx"),
            "no media query for the base ratio: {css}"
        );
    });
}

#[test]
fn spacing_is_in_css_pixels() {
    // Review 3.9: spacing was multiplied by the smallest ratio while images were at the
    // highest one.
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        let manager = run(&Config {
            padding: Some(String::from("2")),
            ..config(&source, &out, "2,1")
        });
        let images = &manager.sprites[0].images;
        // a is 30 CSS px tall + 2 × 2 px of padding.
        assert_eq!(images[1].y, 34);
        // The canvas ends with b (34 + 20 = 54 CSS px), i.e. 108 px at 2×.
        assert_eq!(dimensions(out.join("icons@2x.png")), (40, 108));
    });
}

#[test]
fn less_and_scss_also_switch_image_per_ratio() {
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        run(&Config {
            less: true,
            scss: true,
            ..config(&source, &out, "2,1")
        });
        for file in ["icons.less", "icons.scss"] {
            let text = std::fs::read_to_string(out.join(file)).unwrap();
            assert!(
                text.contains("background-size: 20px 50px;"),
                "{file}: {text}"
            );
            assert!(text.contains("url('icons@2x.png')"), "{file}: {text}");
        }
    });
}

#[test]
fn without_1x_the_smallest_ratio_is_the_default_image() {
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        run(&config(&source, &out, "2"));
        assert!(!out.join("icons.png").exists());
        assert_eq!(dimensions(out.join("icons@2x.png")), (40, 100));
        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert!(css.contains("url('icons@2x.png')"), "{css}");
        assert!(css.contains("background-size: 20px 50px;"), "{css}");
        assert!(!css.contains("@media"), "{css}");
    });
}

#[test]
fn fractional_ratio_image_is_rounded_up() {
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        let manager = run(&config(&source, &out, "1.5,1"));
        // 40×60 px at 1.5× → ceil(26.7)×40 CSS px; b → 27×27.
        assert_eq!(manager.sprites[0].dimensions(), (27, 67));
        assert_eq!(dimensions(out.join("icons.png")), (27, 67));
        // The highest ratio keeps the source pixels: ceil(27 × 1.5) × ceil(67 × 1.5).
        assert_eq!(dimensions(out.join("icons@1.5x.png")), (41, 101));
    });
}

#[test]
fn default_output_has_no_retina_rules() {
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        run(&Config {
            ratios: None,
            ..config(&source, &out, "1")
        });
        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert!(!css.contains("background-size"), "{css}");
        assert!(!css.contains("@media"), "{css}");
        assert_eq!(dimensions(out.join("icons.png")), (40, 100));
        assert!(!out.join("icons@2x.png").exists());
    });
}

#[test]
fn hashed_names_per_ratio_reference_existing_files() {
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        let hashed = Config {
            cachebuster_filename: true,
            ..config(&source, &out, "2,1")
        };
        run(&hashed);
        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        let urls: Vec<&str> = css
            .split("url('")
            .skip(1)
            .map(|rest| &rest[..rest.find('\'').unwrap()])
            .collect();
        assert_eq!(urls.len(), 2, "{css}");
        assert!(urls[1].ends_with("@2x.png"), "{css}");
        for url in &urls {
            assert!(out.join(url).exists(), "missing {url}");
        }

        // A new image replaces both hashed files.
        image::RgbaImage::from_pixel(40, 40, image::Rgba([0, 255, 0, 255]))
            .save(source.join("b.png"))
            .unwrap();
        run(&hashed);
        let sprites = std::fs::read_dir(&out)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".png"))
            .count();
        assert_eq!(sprites, 2, "stale hashed images must be removed");
    });
}

#[test]
fn missing_ratio_image_triggers_regeneration() {
    with_tmp_dir(|tmp| {
        let (source, out) = retina_icons(tmp);
        let incremental = Config {
            force: false,
            ..config(&source, &out, "2,1")
        };
        run(&incremental);
        std::fs::remove_file(out.join("icons@2x.png")).unwrap();

        let mut manager = Manager {
            config: incremental.clone(),
            ..Default::default()
        };
        manager.find_sprites();
        assert!(!manager.is_up_to_date());
        manager.save();
        assert!(out.join("icons@2x.png").exists());
    });
}

#[test]
fn ratio_images_are_not_read_back_as_sources() {
    with_tmp_dir(|tmp| {
        let (source, _) = retina_icons(tmp);
        let same = config(&source, &source, "2,1");
        run(&same);
        let manager = run(&same);
        assert_eq!(manager.sprites[0].images.len(), 2);
    });
}
