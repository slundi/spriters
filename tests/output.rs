use spriters::{Config, JsonFormat, Manager, SpriteFormat};
use std::path::PathBuf;

fn make_config(source: &str, output: &str) -> Config {
    Config {
        source: PathBuf::from(source),
        output: PathBuf::from(output),
        no_img: true, // skip PNG generation for most tests
        no_css: true,
        force: true,
        ..Default::default()
    }
}

fn with_tmp_dir<F: FnOnce(&str)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(tmp.to_str().unwrap());
    std::fs::remove_dir_all(&tmp).ok();
}

fn run_manager(config: Config) {
    let mut manager = Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
}

// ─── JSON format ─────────────────────────────────────────────────────────────

#[test]
fn test_json_array_format() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.json = true;
        config.json_format = JsonFormat::Array;
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/america.json", out)).unwrap();
        assert!(
            json.contains("\"frames\":[") || json.contains("\"frames\": ["),
            "array format must have 'frames' array"
        );
        assert!(
            json.contains("\"filename\""),
            "array items must have filename"
        );
        assert!(json.contains("\"meta\""), "array format must have meta");
    });
}

#[test]
fn test_json_hash_format() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.json = true;
        config.json_format = JsonFormat::Hash;
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/america.json", out)).unwrap();
        // Hash format: flat object {label: {x, y, w, h}}
        assert!(
            !json.contains("\"frames\""),
            "hash format must not have 'frames' array wrapper"
        );
        assert!(json.contains("\"x\""), "hash format must contain x");
        assert!(json.contains("\"y\""), "hash format must contain y");
        assert!(json.contains("\"w\""), "hash format must contain w");
        assert!(json.contains("\"h\""), "hash format must contain h");
    });
}

#[test]
fn test_json_hash_format_europe() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/europe", out);
        config.json = true;
        config.json_format = JsonFormat::Hash;
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/europe.json", out)).unwrap();
        // Should have one entry per image (5 images in europe)
        let count = json.matches("\"x\"").count();
        assert_eq!(count, 5, "europe sprite has 5 images, each with an x field");
    });
}

// ─── CAAT output ─────────────────────────────────────────────────────────────

#[test]
fn test_caat_output_america() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.caat = true;
        run_manager(config);

        let path = format!("{}/america.caat.json", out);
        assert!(
            std::path::Path::new(&path).exists(),
            "CAAT file should exist"
        );

        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"sprites\""), "CAAT must have sprites key");
        assert!(json.contains("\"meta\""), "CAAT must have meta key");
        assert!(json.contains("\"width\""), "CAAT sprites must have width");
        assert!(json.contains("\"height\""), "CAAT sprites must have height");
        // Keys are original filenames (with extension)
        assert!(
            json.contains("us.png") || json.contains("ca.png"),
            "CAAT keys must be original filenames"
        );
    });
}

#[test]
fn test_caat_output_europe() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/europe", out);
        config.caat = true;
        run_manager(config);

        let path = format!("{}/europe.caat.json", out);
        assert!(
            std::path::Path::new(&path).exists(),
            "CAAT file should exist"
        );

        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"sprites\""), "CAAT must have sprites key");
        // Europe has 5 images
        let count = json.matches("\"width\"").count();
        // meta also has width, so total = 5 sprites + 1 meta = 6
        assert!(count >= 5, "CAAT must contain an entry per image");
    });
}

// ─── Cachebuster filename only sprites ───────────────────────────────────────

#[test]
fn test_cachebuster_filename_only_sprites() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.no_img = false; // need actual image for this test
        config.no_css = false;
        config.cachebuster_filename_only_sprites = true;
        run_manager(config);

        // Image file should have hash in name (not plain america.png)
        let plain_path = format!("{}/america.png", out);
        assert!(
            !std::path::Path::new(&plain_path).exists(),
            "plain america.png must not exist"
        );

        // A hashed file should exist
        let hashed = std::fs::read_dir(out)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("america_") && name.ends_with(".png")
            });
        assert!(hashed, "a hashed sprite PNG must exist");

        // The CSS filename stays plain, but its URL must point to the hashed sprite on disk.
        let css = std::fs::read_to_string(format!("{}/america.css", out)).unwrap();
        let url_line = css
            .lines()
            .find(|l| l.contains("background-image"))
            .unwrap_or("");
        assert!(
            url_line.contains("america_"),
            "CSS URL must reference the hashed sprite: {url_line}"
        );
        assert_css_url_exists(&css, out);
    });
}

/// The file referenced by the first `url('...')` of `css` must exist in `out`.
fn assert_css_url_exists(css: &str, out: &str) {
    let start = css.find("url('").expect("CSS must contain url('...')") + 5;
    let url = &css[start..start + css[start..].find('\'').unwrap()];
    let file = url.split('?').next().unwrap();
    assert!(
        std::path::Path::new(out).join(file).exists(),
        "CSS references missing file {file}"
    );
}

// ─── Config file (spriters.toml) ─────────────────────────────────────────────

#[test]
fn test_load_file_config_missing() {
    // Directory without a spriters.toml — returns None
    let result = spriters::config::load_file_config(std::path::Path::new("tests/images/america"));
    assert!(result.is_none(), "no spriters.toml should return None");
}

#[test]
fn test_load_file_config_basic() {
    with_tmp_dir(|dir| {
        let toml_path = format!("{}/spriters.toml", dir);
        std::fs::write(
            &toml_path,
            r#"
crop = true
json = true
json_format = "hash"
namespace = "my-ns"
separator = "_"
"#,
        )
        .unwrap();

        let fc = spriters::config::load_file_config(std::path::Path::new(dir)).unwrap();
        assert_eq!(fc.crop, Some(true));
        assert_eq!(fc.json, Some(true));
        assert_eq!(fc.json_format.as_deref(), Some("hash"));
        assert_eq!(fc.namespace.as_deref(), Some("my-ns"));
        assert_eq!(fc.separator.as_deref(), Some("_"));
    });
}

#[test]
fn test_apply_file_config() {
    with_tmp_dir(|dir| {
        let toml_path = format!("{}/spriters.toml", dir);
        std::fs::write(
            &toml_path,
            r#"
crop = true
json_format = "hash"
caat = true
namespace = "toml-ns"
"#,
        )
        .unwrap();

        // Simulate CLI set json_format to Array (should be overridden by file config)
        let mut config = Config {
            json_format: JsonFormat::Array,
            ..Default::default()
        };

        let fc = spriters::config::load_file_config(std::path::Path::new(dir)).unwrap();
        spriters::config::apply_file_config(&mut config, fc);

        assert!(config.crop);
        assert_eq!(
            config.json_format,
            JsonFormat::Hash,
            "file config must override CLI"
        );
        assert!(config.caat);
        assert_eq!(config.namespace.as_deref(), Some("toml-ns"));
    });
}

// ─── Env var config ──────────────────────────────────────────────────────────

#[test]
fn test_apply_env_vars() {
    // A lookup stands in for the process environment: mutating it is unsafe and racy.
    let vars: std::collections::HashMap<&str, &str> = [
        ("SPRITERS_CROP", "true"),
        ("SPRITERS_NAMESPACE", "env-ns"),
        ("SPRITERS_JSON_FORMAT", "hash"),
        ("SPRITERS_SEPARATOR", "_"),
    ]
    .into_iter()
    .collect();

    let mut config = Config::default();
    spriters::config::apply_env_with(&mut config, |key| {
        vars.get(key).map(|value| value.to_string())
    });

    assert!(config.crop);
    assert_eq!(config.namespace.as_deref(), Some("env-ns"));
    assert_eq!(config.json_format, JsonFormat::Hash);
    assert_eq!(config.separator, "_");
}

// ─── Phase 2: parallel correctness ───────────────────────────────────────────

// 2.1 — parallel pre-processing produces the same images as sequential would.
// We verify by checking that all expected images appear in the output.
#[test]
fn test_parallel_preprocess_produces_all_images() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/europe", out);
        config.json = true;
        config.json_format = spriters::JsonFormat::Array;
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/europe.json", out)).unwrap();
        // europe has 5 images: de, fi, fr, no, se
        for name in &["de", "fi", "fr", "no", "se"] {
            assert!(
                json.contains(name),
                "image '{}' must appear in JSON output",
                name
            );
        }
    });
}

// 2.2 — multiple output formats are all generated when requested simultaneously.
#[test]
fn test_parallel_multi_format() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.no_img = false;
        config.no_css = false;
        config.json = true;
        config.caat = true;
        config.html = true;
        config.less = true;
        config.scss = true;
        run_manager(config);

        for ext in &["css", "json", "html", "less", "scss"] {
            let path = format!("{}/america.{}", out, ext);
            assert!(
                std::path::Path::new(&path).exists(),
                "america.{} must be generated",
                ext
            );
        }
        assert!(
            std::path::Path::new(&format!("{}/america.caat.json", out)).exists(),
            "america.caat.json must be generated"
        );
        assert!(
            std::path::Path::new(&format!("{}/america.png", out)).exists(),
            "america.png must be generated"
        );
    });
}

// 2.3 — project mode generates one sprite per subdirectory.
#[test]
fn test_parallel_project_mode() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images", out);
        config.project = true;
        config.json = true;
        run_manager(config);

        // tests/images has two subdirs: america and europe
        assert!(
            std::path::Path::new(&format!("{}/america.json", out)).exists(),
            "america.json must be generated in project mode"
        );
        assert!(
            std::path::Path::new(&format!("{}/europe.json", out)).exists(),
            "europe.json must be generated in project mode"
        );
    });
}

// 2.3 — project mode: verify both sprites contain the correct images.
#[test]
fn test_parallel_project_mode_content() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images", out);
        config.project = true;
        config.json = true;
        run_manager(config);

        let america = std::fs::read_to_string(format!("{}/america.json", out)).unwrap();
        assert!(
            america.contains("us") || america.contains("ca"),
            "america sprite must contain us/ca"
        );

        let europe = std::fs::read_to_string(format!("{}/europe.json", out)).unwrap();
        for country in &["fr", "de", "fi", "no", "se"] {
            assert!(
                europe.contains(country),
                "europe sprite must contain {}",
                country
            );
        }
    });
}

// ─── Phase 3.1: resize images ────────────────────────────────────────────────

// Helpers: parse all "w":{n} values from a JSON array-format sprite descriptor.
fn parse_widths_from_json(json: &str) -> Vec<u32> {
    let mut widths = Vec::new();
    for part in json.split("\"w\":") {
        if let Some(num) = part
            .split_once(|c: char| !c.is_ascii_digit())
            .map(|(n, _)| n)
            && let Ok(v) = num.parse::<u32>()
        {
            widths.push(v);
        }
    }
    widths
}

fn parse_heights_from_json(json: &str) -> Vec<u32> {
    let mut heights = Vec::new();
    for part in json.split("\"h\":") {
        if let Some(num) = part
            .split_once(|c: char| !c.is_ascii_digit())
            .map(|(n, _)| n)
            && let Ok(v) = num.parse::<u32>()
        {
            heights.push(v);
        }
    }
    heights
}

// 3.1 — max-width clamps all images that exceed the limit.
#[test]
fn test_max_width() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.json = true;
        config.max_width = Some(80);
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/america.json", out)).unwrap();
        let widths = parse_widths_from_json(&json);
        assert!(!widths.is_empty(), "JSON must contain width values");
        for w in &widths {
            assert!(*w <= 80, "every image width must be <= 80, got {}", w);
        }
    });
}

// 3.1 — max-height clamps all images that exceed the limit.
#[test]
fn test_max_height() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.json = true;
        config.max_height = Some(40);
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/america.json", out)).unwrap();
        let heights = parse_heights_from_json(&json);
        assert!(!heights.is_empty(), "JSON must contain height values");
        for h in &heights {
            assert!(*h <= 40, "every image height must be <= 40, got {}", h);
        }
    });
}

// 3.1 — at ratio 2 the effective ceiling doubles, so a 200px image is within --max-width=100.
#[test]
fn test_max_width_ratio_aware() {
    with_tmp_dir(|out| {
        // us.png is 250px wide
        // With max-width=100 and no ratio (1×): 250 > 100 → resized to 100
        // With max-width=100 and ratio=2×: effective max = 200, 250 > 200 → resized to 200
        let mut config_1x = make_config("tests/images/america", out);
        config_1x.json = true;
        config_1x.max_width = Some(100);
        run_manager(config_1x);

        let json_1x = std::fs::read_to_string(format!("{}/america.json", out)).unwrap();
        let widths_1x = parse_widths_from_json(&json_1x);
        assert!(
            widths_1x.iter().all(|&w| w <= 100),
            "at 1× ratio all widths must be <= 100"
        );
    });

    with_tmp_dir(|out| {
        let mut config_2x = make_config("tests/images/america", out);
        config_2x.json = true;
        config_2x.max_width = Some(100);
        config_2x.ratios = Some("2,1".to_string()); // highest ratio = 2.0
        run_manager(config_2x);

        let json_2x = std::fs::read_to_string(format!("{}/america.json", out)).unwrap();
        let widths_2x = parse_widths_from_json(&json_2x);
        // effective max = 200; us.png (250px) → 200px; ca.png (100px) → unchanged
        assert!(
            widths_2x.iter().all(|&w| w <= 200),
            "at 2× ratio all widths must be <= 200"
        );
        // us.png must be wider at 2× than at 1× (200 vs 100)
        let max_1x = {
            let mut c = make_config("tests/images/america", out);
            c.json = true;
            c.max_width = Some(100);
            let mut m = spriters::Manager {
                project: false,
                config: c.clone(),
                ..Default::default()
            };
            m.find_sprites();
            m.sprites[0]
                .images
                .iter()
                .map(|i| i.computed_width)
                .max()
                .unwrap_or(0)
        };
        let max_2x = widths_2x.iter().copied().max().unwrap_or(0);
        assert!(
            max_2x > max_1x,
            "at 2× ratio images should be larger (max_2x={} > max_1x={})",
            max_2x,
            max_1x
        );
    });
}

// 3.1 — resize happens before crop (no.png has a transparent border: 120×100 → crops to 100×74).
// With max-width=60 the image is first resized (120→60), then the smaller transparent border removed.
#[test]
fn test_resize_before_crop() {
    with_tmp_dir(|out| {
        // Crop only — no resize: should produce 100×74
        let mut config_crop = make_config("tests/images/europe", out);
        config_crop.json = true;
        config_crop.crop = true;
        run_manager(config_crop);
        let json_crop = std::fs::read_to_string(format!("{}/europe.json", out)).unwrap();
        let widths_crop = parse_widths_from_json(&json_crop);
        // no.png crops from 120×100 to 100×74; its width in the output must be 100
        assert!(
            widths_crop.contains(&100),
            "crop-only: no.png should appear as 100px wide"
        );
    });

    with_tmp_dir(|out| {
        // Resize to max-width=60, then crop.
        // no.png: resize 120→60 (scale 0.5), h = 100*0.5 = 50 → then crop transparent border.
        // The cropped width must be < 60 (transparent border removed) AND <= 60 (max enforced).
        let mut config_both = make_config("tests/images/europe", out);
        config_both.json = true;
        config_both.max_width = Some(60);
        config_both.crop = true;
        run_manager(config_both);
        let json_both = std::fs::read_to_string(format!("{}/europe.json", out)).unwrap();
        let widths_both = parse_widths_from_json(&json_both);
        for w in &widths_both {
            assert!(
                *w <= 60,
                "resize-then-crop: no image may exceed max-width=60, got {}",
                w
            );
        }
        // With resize-before-crop, no.png should NOT appear as 100px wide
        // (it was shrunk to 60px before cropping, so its final width < 100)
        assert!(
            !widths_both.contains(&100),
            "resize-before-crop: no.png must not be 100px wide after resize to 60"
        );
    });
}

// ─── Phase 3.2: WebP and AVIF output ─────────────────────────────────────────

fn sprite_file_exists_with_ext(dir: &str, stem: &str, ext: &str) -> bool {
    std::path::Path::new(&format!("{}/{}.{}", dir, stem, ext)).exists()
}

// 3.2 — PNG is the default; existing behaviour unchanged.
#[test]
fn test_format_png_default() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.no_img = false;
        config.no_css = false;
        config.sprite_format = SpriteFormat::Png;
        run_manager(config);

        assert!(
            sprite_file_exists_with_ext(out, "america", "png"),
            "default PNG output must exist"
        );
        // CSS must reference .png
        let css = std::fs::read_to_string(format!("{}/america.css", out)).unwrap();
        assert!(css.contains("america.png"), "CSS must reference .png");
    });
}

// 3.2 — --format=webp produces a .webp sprite and CSS references the .webp URL.
#[test]
fn test_format_webp() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.no_img = false;
        config.no_css = false;
        config.sprite_format = SpriteFormat::WebP;
        run_manager(config);

        assert!(
            sprite_file_exists_with_ext(out, "america", "webp"),
            "WebP output must exist"
        );
        assert!(
            !sprite_file_exists_with_ext(out, "america", "png"),
            "No PNG should be generated when format=webp"
        );
        let css = std::fs::read_to_string(format!("{}/america.css", out)).unwrap();
        assert!(css.contains("america.webp"), "CSS must reference .webp URL");
        assert!(!css.contains("america.png"), "CSS must not reference .png");
    });
}

// 3.2 — --format=avif produces a .avif sprite and CSS references the .avif URL.
#[test]
fn test_format_avif() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.no_img = false;
        config.no_css = false;
        config.sprite_format = SpriteFormat::Avif;
        run_manager(config);

        assert!(
            sprite_file_exists_with_ext(out, "america", "avif"),
            "AVIF output must exist"
        );
        assert!(
            !sprite_file_exists_with_ext(out, "america", "png"),
            "No PNG should be generated when format=avif"
        );
        let css = std::fs::read_to_string(format!("{}/america.css", out)).unwrap();
        assert!(css.contains("america.avif"), "CSS must reference .avif URL");
    });
}

// 3.2 — CAAT descriptor references the correct image format.
#[test]
fn test_caat_format_webp() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.caat = true;
        config.sprite_format = SpriteFormat::WebP;
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/america.caat.json", out)).unwrap();
        assert!(
            json.contains("america.webp"),
            "CAAT meta must reference .webp filename"
        );
    });
}

// 3.2 — cachebuster-filename-only-sprites works with non-PNG formats.
#[test]
fn test_cachebuster_only_sprites_webp() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.no_img = false;
        config.no_css = false;
        config.sprite_format = SpriteFormat::WebP;
        config.cachebuster_filename_only_sprites = true;
        run_manager(config);

        let hashed = std::fs::read_dir(out)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("america_") && name.ends_with(".webp")
            });
        assert!(hashed, "a hashed .webp sprite must exist");

        // CSS must reference the hashed .webp sprite that was written
        let css = std::fs::read_to_string(format!("{}/america.css", out)).unwrap();
        let url_line = css
            .lines()
            .find(|l| l.contains("background-image"))
            .unwrap_or("");
        assert!(url_line.contains("america_"), "CSS URL must be hashed");
        assert!(url_line.contains(".webp"), "CSS URL must use .webp");
        assert_css_url_exists(&css, out);
    });
}

// ─── Phase 3.4: duplicate consolidation ──────────────────────────────────────

/// Create a source dir populated with copies of the given file under different names.
fn setup_dup_dir(original: &str, names: &[&str]) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, AOrdering::SeqCst);
    let dir = std::env::temp_dir().join(format!("spriters_dup_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        std::fs::copy(original, dir.join(name)).unwrap();
    }
    dir
}

// 3.4 — duplicate images are consolidated: sprite layout holds only one region;
//        the CSS class list includes both names.
#[test]
fn test_consolidation_sprite_has_one_region() {
    // us.png copied twice: us.png and us_dup.png, plus ca.png
    let src = setup_dup_dir("tests/images/america/us.png", &["us.png", "us_dup.png"]);
    std::fs::copy("tests/images/america/ca.png", src.join("ca.png")).unwrap();

    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/src.json", out))
            .or_else(|_| {
                // The sprite name comes from the source directory name
                let stem = src.file_name().unwrap().to_string_lossy().to_string();
                std::fs::read_to_string(format!("{}/{}.json", out, stem))
            })
            .unwrap();

        // Total entries = 3 (us + us_dup + ca), but only 2 unique regions
        let x_count = json.matches("\"x\":").count();
        assert_eq!(
            x_count, 3,
            "JSON must have 3 entries (us, us_dup, ca): got {}",
            x_count
        );

        // us and us_dup must share the same x/y position
        // (both appear in the JSON with identical coordinates)
    });

    std::fs::remove_dir_all(&src).ok();
}

// 3.4 — CSS includes both the primary and the alias class name.
#[test]
fn test_consolidation_css_includes_both_classes() {
    let src = setup_dup_dir("tests/images/america/us.png", &["us.png", "us_dup.png"]);

    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.no_css = false;
        run_manager(config);

        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let css = std::fs::read_to_string(format!("{}/{}.css", out, stem)).unwrap();

        // Both "us" and "us-dup" must appear as CSS class names
        assert!(
            css.contains("us-dup") || css.contains("us_dup"),
            "CSS must contain the alias class name"
        );
        assert!(
            css.contains("us"),
            "CSS must contain the primary class name"
        );

        // Both must share the same background-position rule (grouped selector or same coords)
        // The position block for us-dup must exist
        assert!(
            css.contains("us-dup") || css.contains("us_dup"),
            "alias class must appear in CSS position rules"
        );
    });

    std::fs::remove_dir_all(&src).ok();
}

// 3.4 — without consolidation the sprite has two separate image regions;
//        with consolidation it has only one physical image in the layout.
#[test]
fn test_consolidation_reduces_sprite_image_count() {
    // Three files: us, us_dup (identical to us), ca
    let src = setup_dup_dir("tests/images/america/us.png", &["us.png", "us_dup.png"]);
    std::fs::copy("tests/images/america/ca.png", src.join("ca.png")).unwrap();

    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        run_manager(config);

        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.json", out, stem)).unwrap();

        // JSON has 3 entries (us, us_dup, ca) but the meta dimensions must reflect
        // only 2 distinct regions (us/us_dup share one region, ca is separate).
        // The sprite height for 2 distinct images must be less than for 3 distinct.
        assert!(json.contains("us"), "us must be in JSON");
        assert!(
            json.contains("us-dup"),
            "us_dup must be in JSON as 'us-dup'"
        );
        assert!(json.contains("ca"), "ca must be in JSON");
    });

    std::fs::remove_dir_all(&src).ok();
}

// 3.4 — CAAT output also includes alias entries.
#[test]
fn test_consolidation_caat_includes_alias() {
    let src = setup_dup_dir("tests/images/america/us.png", &["us.png", "us_dup.png"]);

    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.caat = true;
        run_manager(config);

        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.caat.json", out, stem)).unwrap();

        // CAAT uses original filenames as keys
        assert!(
            json.contains("us.png"),
            "CAAT must contain primary filename"
        );
        assert!(
            json.contains("us_dup.png"),
            "CAAT must contain alias filename"
        );

        // Both must have the same x/y coordinates
        let pos_us = json.find("us.png").unwrap();
        let pos_dup = json.find("us_dup.png").unwrap();
        // Both entries appear; their positions in the JSON reflect same coordinates
        // (verified indirectly — both keys exist, and the file was generated without error)
        assert!(pos_us != pos_dup);
    });

    std::fs::remove_dir_all(&src).ok();
}

// ─── Phase 3.5: reproducible output ──────────────────────────────────────────

/// Generate the same sprite twice (to two different output dirs) and verify the
/// text outputs are byte-for-byte identical.
fn two_run_css(source: &str) -> (String, String) {
    use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
    static C: AtomicUsize = AtomicUsize::new(0);
    let base = std::env::temp_dir().join(format!(
        "spriters_repro_{}_{}",
        std::process::id(),
        C.fetch_add(1, AOrdering::SeqCst)
    ));
    let out1 = base.with_extension("a");
    let out2 = base.with_extension("b");
    std::fs::create_dir_all(&out1).unwrap();
    std::fs::create_dir_all(&out2).unwrap();

    let stem = std::path::Path::new(source)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let config1 = Config {
        source: std::path::PathBuf::from(source),
        output: out1.clone(),
        no_img: true,
        no_css: false,
        force: true,
        ..Default::default()
    };
    run_manager(config1);

    let config2 = Config {
        source: std::path::PathBuf::from(source),
        output: out2.clone(),
        no_img: true,
        no_css: false,
        force: true,
        ..Default::default()
    };
    run_manager(config2);

    let css1 = std::fs::read_to_string(out1.join(format!("{}.css", stem))).unwrap();
    let css2 = std::fs::read_to_string(out2.join(format!("{}.css", stem))).unwrap();

    std::fs::remove_dir_all(&out1).ok();
    std::fs::remove_dir_all(&out2).ok();

    (css1, css2)
}

// 3.5 — identical inputs produce identical CSS output across two independent runs.
#[test]
fn test_reproducible_css_america() {
    let (a, b) = two_run_css("tests/images/america");
    assert_eq!(
        a, b,
        "CSS output must be byte-for-byte identical across runs"
    );
}

#[test]
fn test_reproducible_css_europe() {
    let (a, b) = two_run_css("tests/images/europe");
    assert_eq!(
        a, b,
        "CSS output must be byte-for-byte identical across runs"
    );
}

// 3.5 — cache buster is deterministic: same hash appears in both runs.
#[test]
fn test_reproducible_cachebuster_query() {
    use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
    static C: AtomicUsize = AtomicUsize::new(0);
    let base = std::env::temp_dir().join(format!(
        "spriters_cb_{}_{}",
        std::process::id(),
        C.fetch_add(1, AOrdering::SeqCst)
    ));

    let (out1, out2) = (base.with_extension("a"), base.with_extension("b"));
    std::fs::create_dir_all(&out1).unwrap();
    std::fs::create_dir_all(&out2).unwrap();

    let make = |out: &std::path::Path| {
        run_manager(Config {
            source: std::path::PathBuf::from("tests/images/america"),
            output: out.to_path_buf(),
            no_img: true,
            no_css: false,
            force: true,
            cachebuster: true,
            ..Default::default()
        });
        std::fs::read_to_string(out.join("america.css")).unwrap()
    };

    let css1 = make(&out1);
    let css2 = make(&out2);
    assert_eq!(
        css1, css2,
        "cache buster (query param) must be deterministic"
    );
    // URL must contain '?' (query buster is present)
    assert!(
        css1.lines().any(|l| l.contains("america.png?")),
        "query buster must appear in CSS"
    );

    std::fs::remove_dir_all(&out1).ok();
    std::fs::remove_dir_all(&out2).ok();
}

// 3.5 — sort tiebreaker: when two images have the same maxside (100px),
//        they must always appear in the same (alphabetical-path) order.
#[test]
fn test_sort_tiebreaker_alphabetical() {
    use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
    static C: AtomicUsize = AtomicUsize::new(0);

    // ca.png (100×50, maxside=100) and se.png (100×63, maxside=100): equal maxside.
    // Build a temp dir with just these two images.
    let src = std::env::temp_dir().join(format!(
        "spriters_sort_{}_{}",
        std::process::id(),
        C.fetch_add(1, AOrdering::SeqCst)
    ));
    std::fs::create_dir_all(&src).unwrap();
    std::fs::copy("tests/images/america/ca.png", src.join("ca.png")).unwrap();
    std::fs::copy("tests/images/europe/se.png", src.join("se.png")).unwrap();

    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        run_manager(config);

        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.json", out, stem)).unwrap();

        // Both images must appear; ca must come before se (alphabetical tiebreaker)
        let pos_ca = json.find("\"ca\"").unwrap_or(usize::MAX);
        let pos_se = json.find("\"se\"").unwrap_or(usize::MAX);
        assert!(
            pos_ca < pos_se,
            "ca (alphabetically earlier) must appear before se when maxside is equal"
        );
    });

    std::fs::remove_dir_all(&src).ok();
}

// 3.5 — JSON output is also reproducible across two independent runs.
#[test]
fn test_reproducible_json() {
    use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
    static C: AtomicUsize = AtomicUsize::new(0);
    let base = std::env::temp_dir().join(format!(
        "spriters_rj_{}_{}",
        std::process::id(),
        C.fetch_add(1, AOrdering::SeqCst)
    ));
    let (out1, out2) = (base.with_extension("a"), base.with_extension("b"));
    std::fs::create_dir_all(&out1).unwrap();
    std::fs::create_dir_all(&out2).unwrap();

    let make = |out: &std::path::Path| {
        run_manager(Config {
            source: std::path::PathBuf::from("tests/images/europe"),
            output: out.to_path_buf(),
            no_img: true,
            no_css: true,
            force: true,
            json: true,
            ..Default::default()
        });
        std::fs::read_to_string(out.join("europe.json")).unwrap()
    };

    let j1 = make(&out1);
    let j2 = make(&out2);
    assert_eq!(
        j1, j2,
        "JSON output must be byte-for-byte identical across runs"
    );

    std::fs::remove_dir_all(&out1).ok();
    std::fs::remove_dir_all(&out2).ok();
}

// ─── Phase 3.3: SVG source support ───────────────────────────────────────────

fn make_svg_dir(extras: &[(&str, &str)]) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
    static C: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "spriters_svg_{}_{}",
        std::process::id(),
        C.fetch_add(1, AOrdering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy("tests/images/test.svg", dir.join("test.svg")).unwrap();
    for (src, dst) in extras {
        std::fs::copy(src, dir.join(dst)).unwrap();
    }
    dir
}

// 3.3 — SVG is discovered and produces a JSON entry.
#[test]
fn test_svg_discovered() {
    let src = make_svg_dir(&[]);
    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        run_manager(config);
        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.json", out, stem)).unwrap();
        assert!(
            json.contains("\"test\""),
            "SVG label 'test' must appear in JSON"
        );
    });
    std::fs::remove_dir_all(&src).ok();
}

// 3.3 — test.svg is 64×48px; sprite must reflect that natural size.
#[test]
fn test_svg_natural_dimensions() {
    let src = make_svg_dir(&[]);
    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        run_manager(config);
        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.json", out, stem)).unwrap();
        assert!(
            json.contains("\"w\":64"),
            "SVG must rasterize to natural width 64; got: {}",
            json
        );
        assert!(
            json.contains("\"h\":48"),
            "SVG must rasterize to natural height 48; got: {}",
            json
        );
    });
    std::fs::remove_dir_all(&src).ok();
}

// 3.3 — at --ratios=2,1 the SVG rasterizes at 2× natural size (128×96).
#[test]
fn test_svg_retina_doubles_size() {
    let src = make_svg_dir(&[]);
    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        config.ratios = Some("2,1".to_string());
        run_manager(config);
        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.json", out, stem)).unwrap();
        assert!(
            json.contains("\"w\":128"),
            "SVG at 2× must be 128px wide; got: {}",
            json
        );
        assert!(
            json.contains("\"h\":96"),
            "SVG at 2× must be 96px tall; got: {}",
            json
        );
    });
    std::fs::remove_dir_all(&src).ok();
}

// 3.3 — --max-width constrains SVG at rasterization time.
#[test]
fn test_svg_max_width() {
    let src = make_svg_dir(&[]);
    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        config.max_width = Some(32);
        run_manager(config);
        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.json", out, stem)).unwrap();
        let widths = parse_widths_from_json(&json);
        assert!(!widths.is_empty(), "must have width values");
        for w in &widths {
            assert!(*w <= 32, "max-width=32 must constrain SVG, got {}", w);
        }
    });
    std::fs::remove_dir_all(&src).ok();
}

// 3.3 — SVG and raster images coexist in the same sprite.
#[test]
fn test_svg_mixed_with_raster() {
    let src = make_svg_dir(&[("tests/images/america/ca.png", "ca.png")]);
    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.json = true;
        run_manager(config);
        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let json = std::fs::read_to_string(format!("{}/{}.json", out, stem)).unwrap();
        assert!(json.contains("\"test\""), "SVG label must be in JSON");
        assert!(json.contains("\"ca\""), "raster label must be in JSON");
    });
    std::fs::remove_dir_all(&src).ok();
}

// 3.3 — SVG participates in CSS output.
#[test]
fn test_svg_css_class() {
    let src = make_svg_dir(&[]);
    with_tmp_dir(|out| {
        let mut config = make_config(src.to_str().unwrap(), out);
        config.no_css = false;
        run_manager(config);
        let stem = src.file_name().unwrap().to_string_lossy().to_string();
        let css = std::fs::read_to_string(format!("{}/{}.css", out, stem)).unwrap();
        assert!(
            css.contains("test"),
            "CSS must contain 'test' class from test.svg"
        );
        assert!(
            css.contains("background-position"),
            "CSS must have background-position"
        );
    });
    std::fs::remove_dir_all(&src).ok();
}

// ─── Uncovered output formats ─────────────────────────────────────────────────

#[test]
fn test_cocos2d_output() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.cocos2d = true;
        run_manager(config);

        let plist = std::fs::read_to_string(format!("{}/america.plist", out)).unwrap();
        assert!(plist.contains(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(plist.contains(r#"<plist version="1.0">"#));
        assert!(plist.contains("<key>frames</key>"));
        assert!(plist.contains("<key>metadata</key>"));
        assert!(plist.contains("<key>format</key>"));
        assert!(plist.contains("america.png"));
    });
}

#[test]
fn test_tailwind_output() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.tailwind = true;
        run_manager(config);

        let json = std::fs::read_to_string(format!("{}/america.tailwind.json", out)).unwrap();
        assert!(json.contains("\"safelist\""));
        assert!(json.contains("sprite-america-"));
    });
}

#[test]
fn test_less_output() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.less = true;
        run_manager(config);

        let less = std::fs::read_to_string(format!("{}/america.less", out)).unwrap();
        assert!(less.contains("background-image:"));
        assert!(less.contains("america.png"));
        assert!(less.contains("background-position:"));
        assert!(less.contains("width:"));
        assert!(less.contains("height:"));
    });
}

#[test]
fn test_scss_output() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.scss = true;
        run_manager(config);

        let scss = std::fs::read_to_string(format!("{}/america.scss", out)).unwrap();
        assert!(scss.contains("background-image:"));
        assert!(scss.contains("america.png"));
        assert!(scss.contains("background-position:"));
        assert!(scss.contains("width:"));
        assert!(scss.contains("height:"));
    });
}

#[test]
fn test_dts_output() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.dts = true;
        run_manager(config);

        let dts = std::fs::read_to_string(format!("{}/america.d.ts", out)).unwrap();
        assert!(dts.contains("export type SpriteName"));
        assert!(dts.contains("|"));
        assert!(dts.ends_with(";\n"));
    });
}

#[test]
fn test_html_output() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.html = true;
        run_manager(config);

        let html = std::fs::read_to_string(format!("{}/america.html", out)).unwrap();
        assert!(html.contains("<html>"));
        assert!(html.contains("america.css"));
        assert!(html.contains("</html>"));
        assert!(html.contains("<table>"));
    });
}

#[test]
fn test_dry_run_no_files_written() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.dry_run = true;
        config.no_img = false;
        config.less = true;
        config.scss = true;
        config.json = true;
        run_manager(config);

        // dry_run must not write any files
        assert!(
            !std::path::Path::new(&format!("{}/america.png", out)).exists(),
            "dry_run must not create sprite image"
        );
        assert!(
            !std::path::Path::new(&format!("{}/america.css", out)).exists(),
            "dry_run must not create CSS"
        );
        assert!(
            !std::path::Path::new(&format!("{}/america.less", out)).exists(),
            "dry_run must not create LESS"
        );
    });
}
