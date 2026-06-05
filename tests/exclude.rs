use spriters::{Config, Sprite};
use std::path::PathBuf;

fn europe_config() -> (Config, PathBuf) {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/images/europe");
    let mut config = Config::default();
    config.source = src.clone();
    config.output = std::env::temp_dir();
    (config, src)
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

// europe/ contains: de.png fi.png fr.png no.png se.png (5 images)

#[test]
fn no_exclude_loads_all_images() {
    let (config, src) = europe_config();
    let sprite = Sprite::new(&config, src);
    assert_eq!(sprite.images.len(), 5);
}

#[test]
fn exact_name_excludes_single_file() {
    let (mut config, src) = europe_config();
    config.exclude = vec!["fr.png".to_owned()];

    let sprite = Sprite::new(&config, src);
    let names = image_names(&sprite);

    assert_eq!(sprite.images.len(), 4);
    assert!(
        !names.contains(&"fr.png"),
        "fr.png should have been excluded"
    );
}

#[test]
fn wildcard_star_excludes_matching_files() {
    // f*.png matches fi.png and fr.png
    let (mut config, src) = europe_config();
    config.exclude = vec!["f*.png".to_owned()];

    let sprite = Sprite::new(&config, src);
    let names = image_names(&sprite);

    assert_eq!(sprite.images.len(), 3);
    assert!(!names.contains(&"fi.png"));
    assert!(!names.contains(&"fr.png"));
    assert!(names.contains(&"de.png"));
    assert!(names.contains(&"no.png"));
    assert!(names.contains(&"se.png"));
}

#[test]
fn wildcard_question_mark_excludes_matching_files() {
    // ?e.png matches de.png and se.png (one char then 'e')
    let (mut config, src) = europe_config();
    config.exclude = vec!["?e.png".to_owned()];

    let sprite = Sprite::new(&config, src);
    let names = image_names(&sprite);

    assert_eq!(sprite.images.len(), 3);
    assert!(!names.contains(&"de.png"));
    assert!(!names.contains(&"se.png"));
    assert!(names.contains(&"fi.png"));
    assert!(names.contains(&"fr.png"));
    assert!(names.contains(&"no.png"));
}

#[test]
fn multiple_exclude_patterns_are_additive() {
    let (mut config, src) = europe_config();
    config.exclude = vec![
        "de.png".to_owned(),
        "se.png".to_owned(),
        "no.png".to_owned(),
    ];

    let sprite = Sprite::new(&config, src);
    let names = image_names(&sprite);

    assert_eq!(sprite.images.len(), 2);
    assert!(names.contains(&"fi.png"));
    assert!(names.contains(&"fr.png"));
}

#[test]
fn pattern_with_no_match_is_a_noop() {
    let (mut config, src) = europe_config();
    config.exclude = vec!["_draft_*".to_owned()];

    let sprite = Sprite::new(&config, src);
    assert_eq!(sprite.images.len(), 5);
}
