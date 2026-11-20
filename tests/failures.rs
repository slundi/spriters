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

/// Save a 4×4 image of `color` at `path`, creating its folder.
fn save_icon(path: &Path, color: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(4, 4, image::Rgba(color))
        .save(path)
        .unwrap();
}

fn file_names(paths: &[String]) -> Vec<&str> {
    paths
        .iter()
        .map(|path| Path::new(path).file_name().unwrap().to_str().unwrap())
        .collect()
}

#[test]
fn colliding_labels_leave_one_image_out() {
    // Review 6.4: `my_image.png` and `my-image.png` both became `.sprite-icons-my-image`,
    // two rules for one class at different positions.
    with_tmp_dir(|tmp| {
        let (source, output) = source_and_output(tmp);
        save_icon(&source.join("my_image.png"), [0, 255, 0, 255]);
        save_icon(&source.join("my-image.png"), [255, 255, 0, 255]);
        let mut manager = manager(&source, &output);

        assert!(!manager.generate());
        let sprite = &manager.sprites[0];
        // The first path is kept: '-' sorts before '_'.
        assert_eq!(file_names(&sprite.failed_images), ["my_image.png"]);
        assert_eq!(sprite.images.len(), 3);
        let css = std::fs::read_to_string(output.join("icons.css")).unwrap();
        assert_eq!(css.matches(".sprite-icons-my-image {").count(), 1, "{css}");
    });
}

#[test]
fn same_file_name_in_two_folders_collides_with_recursive() {
    with_tmp_dir(|tmp| {
        let (source, output) = source_and_output(tmp);
        save_icon(&source.join("rss.png"), [0, 255, 0, 255]);
        save_icon(&source.join("blog").join("rss.png"), [255, 255, 0, 255]);
        let mut manager = manager(&source, &output);
        manager.config.recursive = true;
        manager.config.json = true;
        manager.config.json_format = spriters::JsonFormat::Hash;

        assert!(!manager.generate());
        assert_eq!(manager.sprites[0].failed_images.len(), 1);
        assert!(manager.sprites[0].failed_images[0].ends_with("rss.png"));
        // `blog/rss.png` sorts first and is kept.
        let kept = &manager.sprites[0].images;
        assert!(kept.iter().any(|image| image.path.contains("blog")));
        let json = std::fs::read_to_string(output.join("icons.json")).unwrap();
        assert_eq!(json.matches("\"rss\":").count(), 1, "{json}");
    });
}

#[test]
fn identical_files_with_the_same_label_are_not_a_collision() {
    with_tmp_dir(|tmp| {
        let (source, output) = source_and_output(tmp);
        save_icon(&source.join("rss.png"), [0, 255, 0, 255]);
        save_icon(&source.join("blog").join("rss.png"), [0, 255, 0, 255]);
        let mut manager = manager(&source, &output);
        manager.config.recursive = true;
        manager.config.json = true;
        manager.config.json_format = spriters::JsonFormat::Hash;

        // Same content, same label: one region, one class.
        assert!(manager.generate());
        assert!(manager.sprites[0].failed_images.is_empty());
        let json = std::fs::read_to_string(output.join("icons.json")).unwrap();
        assert_eq!(json.matches("\"rss\":").count(), 1, "{json}");
    });
}
