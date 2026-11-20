use spriters::{Config, Sprite};
use std::path::PathBuf;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn image(rel: &str) -> PathBuf {
    manifest().join("tests/images").join(rel)
}

fn base_config() -> Config {
    // source is not walked in file-list mode, but must satisfy Config::check()
    // via the files.is_empty() guard — so we can leave it as the default CWD.
    Config {
        output: std::env::temp_dir(),
        ..Default::default()
    }
}

fn image_names(sprite: &Sprite) -> Vec<&str> {
    sprite
        .images
        .iter()
        .map(|i| {
            std::path::Path::new(&i.path)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
        })
        .collect()
}

#[test]
fn explicit_file_list_loads_only_listed_images() {
    let mut config = base_config();
    config.files = vec![
        image("europe/fr.png"),
        image("europe/no.png"),
        image("america/us.png"),
    ];
    // Use a stable source path for the sprite name; doesn't affect file loading.
    config.source = manifest().join("tests/images/europe");

    let sprite = Sprite::new(&config, config.source.clone());
    let names = image_names(&sprite);

    assert_eq!(sprite.images.len(), 3);
    assert!(names.contains(&"fr.png"));
    assert!(names.contains(&"no.png"));
    assert!(names.contains(&"us.png"));
}

#[test]
fn file_list_can_span_multiple_directories() {
    let mut config = base_config();
    config.files = vec![image("europe/de.png"), image("america/ca.png")];
    config.source = manifest().join("tests/images/europe");

    let sprite = Sprite::new(&config, config.source.clone());
    let names = image_names(&sprite);

    assert_eq!(sprite.images.len(), 2);
    assert!(names.contains(&"de.png"));
    assert!(names.contains(&"ca.png"));
}

#[test]
fn non_image_paths_in_file_list_are_skipped() {
    let mut config = base_config();
    config.files = vec![
        image("europe/fr.png"),
        manifest().join("README.md"), // not an image
    ];
    config.source = manifest().join("tests/images/europe");

    let sprite = Sprite::new(&config, config.source.clone());

    assert_eq!(sprite.images.len(), 1);
    assert_eq!(image_names(&sprite), vec!["fr.png"]);
}

#[test]
fn exclude_patterns_apply_in_file_list_mode() {
    let mut config = base_config();
    config.files = vec![
        image("europe/fr.png"),
        image("europe/fi.png"),
        image("europe/no.png"),
    ];
    config.source = manifest().join("tests/images/europe");
    config.exclude = vec!["f*.png".to_owned()]; // excludes fr.png and fi.png

    let sprite = Sprite::new(&config, config.source.clone());
    let names = image_names(&sprite);

    assert_eq!(sprite.images.len(), 1);
    assert!(names.contains(&"no.png"));
}

#[test]
fn duplicate_files_in_list_are_consolidated() {
    let mut config = base_config();
    // identical-fr.png is a pixel-identical copy of fr.png
    config.files = vec![image("europe/fr.png"), image("identical-fr.png")];
    config.source = manifest().join("tests/images/europe");

    let sprite = Sprite::new(&config, config.source.clone());

    // Only one sprite region; the duplicate becomes an alias.
    assert_eq!(sprite.images.len(), 1);
    assert_eq!(sprite.images[0].aliases.len(), 1);
}
