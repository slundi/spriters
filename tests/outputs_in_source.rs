//! Generated files must never be read back as source images (review 3.5).

use spriters::{Config, Manager, OutputFilter, SpriteFormat};
use std::path::{Path, PathBuf};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_outsrc_{}_{}",
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

fn write_icon(path: &Path, color: [u8; 4], width: u32, height: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(width, height, image::Rgba(color))
        .save(path)
        .unwrap();
}

/// `<root>/<name>` with two icons (40×30 and 40×30, distinct colours).
fn icons_dir(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    write_icon(&dir.join("a.png"), [255, 0, 0, 255], 40, 30);
    write_icon(&dir.join("b.png"), [0, 0, 255, 255], 40, 30);
    dir
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

fn image_counts(manager: &Manager) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = manager
        .sprites
        .iter()
        .map(|sprite| {
            let name = sprite
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            (name, sprite.images.len())
        })
        .collect();
    counts.sort();
    counts
}

/// Run twice (forced) and check the second run sees exactly the same images.
fn assert_stable(config: &Config) -> Manager {
    let first = run(config);
    let second = run(config);
    assert_eq!(
        image_counts(&first),
        image_counts(&second),
        "generated files were picked up as source images"
    );
    assert_eq!(
        first
            .sprites
            .iter()
            .map(|s| s.dimensions())
            .collect::<Vec<_>>(),
        second
            .sprites
            .iter()
            .map(|s| s.dimensions())
            .collect::<Vec<_>>(),
        "the sprite must not grow between runs"
    );
    second
}

#[test]
fn same_source_and_output_is_stable() {
    // Review 3.5: `same.png` was added to itself on the second run (40x60 -> 40x120).
    with_tmp_dir(|tmp| {
        let dir = icons_dir(tmp, "same");
        let config = Config {
            source: dir.clone(),
            output: dir.clone(),
            force: true,
            json: true,
            ..Default::default()
        };
        let manager = assert_stable(&config);
        assert_eq!(image_counts(&manager), vec![(String::from("same"), 2)]);
    });
}

#[test]
fn same_source_and_output_is_stable_for_every_sprite_name() {
    for (format, hashed) in [
        (SpriteFormat::Png, true),
        (SpriteFormat::WebP, false),
        (SpriteFormat::WebP, true),
    ] {
        with_tmp_dir(|tmp| {
            let dir = icons_dir(tmp, "same");
            let config = Config {
                source: dir.clone(),
                output: dir.clone(),
                force: true,
                sprite_format: format,
                cachebuster_filename_only_sprites: hashed,
                ..Default::default()
            };
            assert_stable(&config);
        });
    }
}

#[test]
fn file_list_mode_skips_generated_sprite() {
    // `fd -e png . | spriters -o .` lists the previous sprite as well.
    with_tmp_dir(|tmp| {
        let dir = icons_dir(tmp, "same");
        let base = Config {
            source: dir.clone(),
            output: dir.clone(),
            force: true,
            ..Default::default()
        };
        run(&base);
        assert!(dir.join("same.png").exists());

        let listed = Config {
            files: vec![dir.join("a.png"), dir.join("b.png"), dir.join("same.png")],
            ..base
        };
        let manager = run(&listed);
        assert_eq!(manager.sprites[0].images.len(), 2);
    });
}

#[test]
fn recursive_skips_output_nested_in_source() {
    with_tmp_dir(|tmp| {
        let dir = icons_dir(tmp, "icons");
        write_icon(&dir.join("sub/c.png"), [0, 255, 0, 255], 8, 8);
        let output = dir.join("dist");
        std::fs::create_dir_all(&output).unwrap();
        let config = Config {
            source: dir.clone(),
            output: output.clone(),
            recursive: true,
            force: true,
            ..Default::default()
        };
        let manager = assert_stable(&config);
        assert_eq!(manager.sprites[0].images.len(), 3);
    });
}

#[test]
fn project_mode_does_not_turn_output_dir_into_a_sprite() {
    with_tmp_dir(|tmp| {
        let root = tmp.join("project");
        icons_dir(&root, "flags");
        icons_dir(&root, "arrows");
        let output = root.join("dist");
        std::fs::create_dir_all(&output).unwrap();
        let config = Config {
            source: root.clone(),
            output: output.clone(),
            project: true,
            force: true,
            ..Default::default()
        };
        let manager = assert_stable(&config);
        assert_eq!(
            image_counts(&manager),
            vec![(String::from("arrows"), 2), (String::from("flags"), 2)]
        );
    });
}

#[test]
fn project_mode_with_output_in_a_sprite_dir_keeps_its_icons() {
    // Output is `flags`: every sprite (`arrows` too) is written there, but the real
    // `flags` icons must stay a sprite and the generated files must not join it.
    with_tmp_dir(|tmp| {
        let root = tmp.join("project");
        let flags = icons_dir(&root, "flags");
        icons_dir(&root, "arrows");
        let config = Config {
            source: root.clone(),
            output: flags.clone(),
            project: true,
            force: true,
            ..Default::default()
        };
        let manager = assert_stable(&config);
        assert!(flags.join("arrows.png").exists());
        assert_eq!(
            image_counts(&manager),
            vec![(String::from("arrows"), 2), (String::from("flags"), 2)]
        );
    });
}

#[test]
fn user_images_in_output_dir_stay_sources() {
    with_tmp_dir(|tmp| {
        let dir = icons_dir(tmp, "icons");
        let output = dir.join("dist");
        write_icon(&output.join("extra.png"), [9, 9, 9, 255], 4, 4);
        let config = Config {
            source: dir.clone(),
            output: output.clone(),
            recursive: true,
            force: true,
            ..Default::default()
        };
        let manager = assert_stable(&config);
        assert_eq!(manager.sprites[0].images.len(), 3, "a, b and dist/extra");
    });
}

#[test]
fn output_filter_for_watch_events() {
    with_tmp_dir(|tmp| {
        let dir = icons_dir(tmp, "same");
        let config = Config {
            source: dir.clone(),
            output: dir.clone(),
            force: true,
            ..Default::default()
        };
        run(&config);
        let outputs = OutputFilter::new(&config);

        // Generated files (existing, or not written yet / just deleted) are outputs...
        assert!(outputs.is_generated_file(&dir.join("same.png")));
        assert!(outputs.is_generated_file(&dir.join("same.css")));
        assert!(outputs.is_generated_file(&dir.join(".same.spriters")));
        assert!(outputs.is_generated_file(&dir.join("same.json")));
        // ...source images are not, even when the output is the source directory.
        assert!(!outputs.is_generated_file(&dir.join("a.png")));
        assert!(!outputs.is_generated_file(&dir.join("new-icon.png")));
    });
}

#[test]
fn output_filter_only_matches_files_in_output_dir() {
    with_tmp_dir(|tmp| {
        let dir = icons_dir(tmp, "icons");
        let output = dir.join("dist");
        std::fs::create_dir_all(&output).unwrap();
        let config = Config {
            source: dir.clone(),
            output: output.clone(),
            ..Default::default()
        };
        let outputs = OutputFilter::new(&config);
        assert!(outputs.is_generated_file(&output.join("icons.png")));
        // Same name outside the output directory: a source icon, not a generated file.
        assert!(!outputs.is_generated_file(&dir.join("icons.png")));
        assert!(!outputs.is_generated_file(&output.join("extra.png")));
        assert!(outputs.is_output_dir(&output));
        assert!(!outputs.is_output_dir(&dir));
    });
}
