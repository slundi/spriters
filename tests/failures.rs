//! Failures reported by the library API (review 6.1): unreadable or corrupt images, empty
//! sprites and files that cannot be written must make `generate`/`save`/`check` return
//! `false`, so that the binary exits with a non-zero code.

use spriters::{Config, Manager};
use std::path::{Path, PathBuf};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_failures_{}_{}",
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

/// Source folder `icons` with a red and a blue icon, and an empty output folder.
fn source_and_output(tmp: &Path) -> (PathBuf, PathBuf) {
    let source = tmp.join("icons");
    let output = tmp.join("out");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for (name, color) in [("red", [255, 0, 0, 255]), ("blue", [0, 0, 255, 255])] {
        image::RgbaImage::from_pixel(4, 4, image::Rgba(color))
            .save(source.join(format!("{name}.png")))
            .unwrap();
    }
    (source, output)
}

fn manager(source: &Path, output: &Path) -> Manager {
    Manager {
        config: Config {
            source: source.to_path_buf(),
            output: output.to_path_buf(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn generate_succeeds_when_everything_is_written() {
    with_tmp_dir(|tmp| {
        let (source, output) = source_and_output(tmp);
        let mut manager = manager(&source, &output);

        assert!(manager.generate());
        assert!(manager.sprites[0].is_complete());
        assert!(manager.sprites[0].failed_images.is_empty());
        // Up to date: nothing to write is not a failure.
        assert!(manager.generate());
        assert!(manager.check());
    });
}

#[test]
fn corrupt_and_unreadable_images_are_recorded() {
    with_tmp_dir(|tmp| {
        let (source, output) = source_and_output(tmp);
        std::fs::write(source.join("broken.png"), b"not a png").unwrap();
        std::fs::create_dir(source.join("folder.png")).unwrap();
        let mut manager = manager(&source, &output);

        assert!(!manager.generate());
        let sprite = &manager.sprites[0];
        assert!(!sprite.is_complete());
        let failed: Vec<&str> = sprite
            .failed_images
            .iter()
            .map(|path| Path::new(path).file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(failed, ["broken.png", "folder.png"]);
        // The other images still make up the sprite.
        assert_eq!(sprite.images.len(), 2);
        assert!(output.join("icons.png").is_file());

        // Up to date on disk, but still built from an incomplete set of images.
        assert!(!manager.check());
    });
}

#[test]
fn empty_sprite_is_a_failure() {
    with_tmp_dir(|tmp| {
        let source = tmp.join("empty");
        std::fs::create_dir_all(&source).unwrap();
        let mut manager = manager(&source, tmp);

        assert!(!manager.generate());
        assert!(!manager.sprites[0].is_complete());
    });
}

#[test]
fn project_without_sprites_is_a_failure() {
    with_tmp_dir(|tmp| {
        let mut manager = manager(tmp, tmp);
        manager.project = true;
        manager.config.project = true;

        assert!(!manager.generate());
        assert!(manager.sprites.is_empty());
    });
}

#[test]
fn unwritable_output_fails() {
    with_tmp_dir(|tmp| {
        let (source, output) = source_and_output(tmp);
        std::fs::create_dir(output.join("icons.png")).unwrap();

        assert!(!manager(&source, &output).generate());
    });
}

#[test]
fn save_reports_failures_of_every_sprite() {
    with_tmp_dir(|tmp| {
        let (source, output) = source_and_output(tmp);
        let mut manager = manager(&source, &output);
        assert!(!manager.save(), "no sprite found yet: nothing saved");

        manager.find_sprites();
        assert!(manager.save());

        std::fs::write(source.join("broken.png"), b"not a png").unwrap();
        manager.find_sprites();
        assert!(!manager.save());
    });
}
