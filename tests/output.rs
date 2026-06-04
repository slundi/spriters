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
