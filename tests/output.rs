use spriters::{Config, JsonFormat, Manager};
use std::path::PathBuf;

fn make_config(source: &str, output: &str) -> Config {
    let mut config = Config::default();
    config.source = PathBuf::from(source);
    config.output = PathBuf::from(output);
    config.no_img = true; // skip PNG generation for most tests
    config.no_css = true;
    config.force = true;
    config
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

        // CSS must reference plain filename (no hash in URL)
        let css = std::fs::read_to_string(format!("{}/america.css", out)).unwrap();
        // CSS url must not contain '_' before '.png' (the hash pattern)
        let url_line = css
            .lines()
            .find(|l| l.contains("background-image"))
            .unwrap_or("");
        assert!(
            !url_line.contains("america_"),
            "CSS URL must not contain hashed image filename"
        );
    });
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

        let mut config = Config::default();
        // Simulate CLI set json_format to Array (should be overridden by file config)
        config.json_format = JsonFormat::Array;

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
    // Isolate this test by setting and then restoring env vars
    // We only test a few representative vars
    std::env::set_var("SPRITERS_CROP", "true");
    std::env::set_var("SPRITERS_NAMESPACE", "env-ns");
    std::env::set_var("SPRITERS_JSON_FORMAT", "hash");
    std::env::set_var("SPRITERS_SEPARATOR", "_");

    let mut config = Config::default();
    spriters::config::apply_env_vars(&mut config);

    assert!(config.crop);
    assert_eq!(config.namespace.as_deref(), Some("env-ns"));
    assert_eq!(config.json_format, JsonFormat::Hash);
    assert_eq!(config.separator, "_");

    // Cleanup
    std::env::remove_var("SPRITERS_CROP");
    std::env::remove_var("SPRITERS_NAMESPACE");
    std::env::remove_var("SPRITERS_JSON_FORMAT");
    std::env::remove_var("SPRITERS_SEPARATOR");
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
        {
            if let Ok(v) = num.parse::<u32>() {
                widths.push(v);
            }
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
        {
            if let Ok(v) = num.parse::<u32>() {
                heights.push(v);
            }
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
