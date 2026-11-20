use spriters::{Config, Manager};
use std::path::PathBuf;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn with_tmp_dir<F: FnOnce(&std::path::Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_p5_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(&tmp);
    std::fs::remove_dir_all(&tmp).ok();
}

fn base_config(source: &str, output: &std::path::Path) -> Config {
    Config {
        source: PathBuf::from(source),
        output: output.to_path_buf(),
        force: true,
        ..Default::default()
    }
}

fn run(config: Config) {
    let mut manager = Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
}

fn read_file(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).expect("file should exist")
}

// ─── 5.1 Tailwind JSON ───────────────────────────────────────────────────────

#[test]
fn tailwind_creates_tailwind_json() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.tailwind = true;
        run(config);
        assert!(
            out.join("america.tailwind.json").exists(),
            "america.tailwind.json must be created"
        );
    });
}

#[test]
fn tailwind_json_has_safelist_key() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.tailwind = true;
        run(config);
        let content = read_file(&out.join("america.tailwind.json"));
        assert!(
            content.contains("\"safelist\""),
            "must contain \"safelist\" key"
        );
        assert!(content.contains('['), "safelist must be an array");
        assert!(content.contains(']'), "safelist must be an array");
    });
}

#[test]
fn tailwind_json_lists_class_names() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.tailwind = true;
        run(config);
        let content = read_file(&out.join("america.tailwind.json"));
        // Default namespace is "sprite", sprite name "america", so classes like
        // "sprite-america-us" (slugified image names).
        assert!(
            content.contains("sprite-america-"),
            "must contain namespaced class names"
        );
    });
}

#[test]
fn tailwind_disabled_does_not_create_file() {
    with_tmp_dir(|out| {
        let config = base_config("tests/images/america", out);
        // tailwind defaults to false
        run(config);
        assert!(
            !out.join("america.tailwind.json").exists(),
            "tailwind.json must not be created without --tailwind"
        );
    });
}

#[test]
fn tailwind_includes_all_images() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/europe", out);
        config.tailwind = true;
        run(config);
        let content = read_file(&out.join("europe.tailwind.json"));
        // europe/ has de, fi, fr, no, se — each must appear
        for name in &["de", "fi", "fr", "no", "se"] {
            assert!(
                content.contains(&format!("sprite-europe-{}", name)),
                "must contain sprite-europe-{}",
                name
            );
        }
    });
}

// ─── 5.2 TypeScript definitions ──────────────────────────────────────────────

#[test]
fn dts_creates_dts_file() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.dts = true;
        run(config);
        assert!(
            out.join("america.d.ts").exists(),
            "america.d.ts must be created"
        );
    });
}

#[test]
fn dts_contains_sprite_name_type() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.dts = true;
        run(config);
        let content = read_file(&out.join("america.d.ts"));
        assert!(
            content.contains("export type SpriteName"),
            "must export SpriteName type"
        );
    });
}

#[test]
fn dts_contains_image_labels() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/europe", out);
        config.dts = true;
        run(config);
        let content = read_file(&out.join("europe.d.ts"));
        for name in &["de", "fi", "fr", "no", "se"] {
            assert!(
                content.contains(&format!("\"{}\"", name)),
                "must contain label \"{}\"",
                name
            );
        }
    });
}

#[test]
fn dts_type_ends_with_semicolon() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.dts = true;
        run(config);
        let content = read_file(&out.join("america.d.ts"));
        assert!(
            content.trim_end().ends_with(';'),
            "type declaration must end with semicolon"
        );
    });
}

#[test]
fn dts_disabled_does_not_create_file() {
    with_tmp_dir(|out| {
        let config = base_config("tests/images/america", out);
        // dts defaults to false
        run(config);
        assert!(
            !out.join("america.d.ts").exists(),
            ".d.ts must not be created without --dts"
        );
    });
}

#[test]
fn tailwind_and_dts_work_together() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.tailwind = true;
        config.dts = true;
        run(config);
        assert!(
            out.join("america.tailwind.json").exists(),
            "tailwind.json must exist"
        );
        assert!(out.join("america.d.ts").exists(), ".d.ts must exist");
    });
}
