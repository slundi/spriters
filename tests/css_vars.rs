use spriters::{Config, Manager};
use std::path::PathBuf;

fn make_config(source: &str, output: &str) -> Config {
    let mut config = Config::default();
    config.source = PathBuf::from(source);
    config.output = PathBuf::from(output);
    config.no_img = true;
    config.no_css = true;
    config.force = true;
    config.css_vars = true;
    config
}

fn with_tmp_dir<F: FnOnce(&str)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_cssvars_{}",
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

#[test]
fn css_vars_file_is_created() {
    with_tmp_dir(|out| {
        run_manager(make_config("tests/images/america", out));
        assert!(
            std::path::Path::new(&format!("{}/america.vars.css", out)).exists(),
            ".vars.css file must be created"
        );
    });
}

#[test]
fn css_vars_contains_root_block() {
    with_tmp_dir(|out| {
        run_manager(make_config("tests/images/america", out));
        let content = std::fs::read_to_string(format!("{}/america.vars.css", out)).unwrap();
        assert!(content.contains(":root {"), "file must open with :root {{");
        assert!(content.ends_with("}\n"), "file must close the :root block");
    });
}

#[test]
fn css_vars_has_image_url_property() {
    with_tmp_dir(|out| {
        run_manager(make_config("tests/images/america", out));
        let content = std::fs::read_to_string(format!("{}/america.vars.css", out)).unwrap();
        assert!(
            content.contains("--america-image: url("),
            "must include the sprite image URL property"
        );
    });
}

#[test]
fn css_vars_has_xywh_properties_for_each_image() {
    with_tmp_dir(|out| {
        run_manager(make_config("tests/images/america", out));
        let content = std::fs::read_to_string(format!("{}/america.vars.css", out)).unwrap();

        // america/ has ca.png and us.png
        for name in &["ca", "us"] {
            let prefix = format!("--sprite-america-{name}-");
            assert!(
                content.contains(&format!("{prefix}x:")),
                "missing {prefix}x"
            );
            assert!(
                content.contains(&format!("{prefix}y:")),
                "missing {prefix}y"
            );
            assert!(
                content.contains(&format!("{prefix}w:")),
                "missing {prefix}w"
            );
            assert!(
                content.contains(&format!("{prefix}h:")),
                "missing {prefix}h"
            );
        }
    });
}

#[test]
fn css_vars_single_image_at_origin_has_zero_position() {
    // A sprite built from one image always places it at (0, 0).
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        // Use file-list mode to pin to a single image.
        config.files =
            vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/images/europe/fr.png")];
        config.source = PathBuf::from("tests/images/europe");

        run_manager(config);

        let content = std::fs::read_to_string(format!("{}/europe.vars.css", out)).unwrap();
        assert!(
            content.contains("--sprite-europe-fr-x: 0;"),
            "single image x must be 0"
        );
        assert!(
            content.contains("--sprite-europe-fr-y: 0;"),
            "single image y must be 0"
        );
        // fr.png is 250×167
        assert!(
            content.contains("--sprite-europe-fr-w: 250px;"),
            "width must be 250px"
        );
        assert!(
            content.contains("--sprite-europe-fr-h: 167px;"),
            "height must be 167px"
        );
    });
}

#[test]
fn css_vars_aliases_get_own_properties() {
    // identical-fr.png is pixel-identical to fr.png; it becomes an alias.
    // Both should appear as separate custom properties at the same coordinates.
    with_tmp_dir(|out| {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut config = make_config("tests/images/europe", out);
        config.files = vec![
            manifest.join("tests/images/europe/fr.png"),
            manifest.join("tests/images/identical-fr.png"),
        ];
        config.source = PathBuf::from("tests/images/europe");

        run_manager(config);

        let content = std::fs::read_to_string(format!("{}/europe.vars.css", out)).unwrap();
        assert!(
            content.contains("--sprite-europe-fr-x:"),
            "primary must have x property"
        );
        assert!(
            content.contains("--sprite-europe-identical-fr-x:"),
            "alias must have its own x property"
        );
    });
}

#[test]
fn css_vars_not_generated_without_flag() {
    with_tmp_dir(|out| {
        let mut config = make_config("tests/images/america", out);
        config.css_vars = false;
        run_manager(config);
        assert!(
            !std::path::Path::new(&format!("{}/america.vars.css", out)).exists(),
            ".vars.css must NOT be created when --css-vars is not set"
        );
    });
}
