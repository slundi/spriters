//! Parallel discovery and one-sprite-at-a-time generation (review 5.1).

use spriters::{Config, Manager};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_stream_{}_{}",
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

fn write_icon(path: &Path, seed: u32) {
    let color = [
        (seed * 37 % 256) as u8,
        (seed * 91 % 256) as u8,
        (seed * 13 % 256) as u8,
        255,
    ];
    image::RgbaImage::from_pixel(4 + seed % 9, 4 + seed % 7, image::Rgba(color))
        .save(path)
        .unwrap();
}

/// A project with three sprites of a few icons each.
fn project(root: &Path) -> PathBuf {
    let project = root.join("project");
    for (sprite, count) in [("arrows", 5u32), ("flags", 8), ("social", 3)] {
        let dir = project.join(sprite);
        std::fs::create_dir_all(&dir).unwrap();
        for index in 0..count {
            write_icon(&dir.join(format!("icon{index}.png")), index + count * 10);
        }
    }
    project
}

fn project_config(source: &Path, output: &Path) -> Config {
    Config {
        source: source.to_path_buf(),
        output: output.to_path_buf(),
        project: true,
        json: true,
        force: true,
        ..Default::default()
    }
}

fn manager(config: &Config) -> Manager {
    Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    }
}

/// Every file of `dir` with its content.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn generate_writes_the_same_files_as_find_and_save() {
    with_tmp_dir(|tmp| {
        let source = project(tmp);
        let (all_at_once, streamed) = (tmp.join("a"), tmp.join("b"));
        std::fs::create_dir_all(&all_at_once).unwrap();
        std::fs::create_dir_all(&streamed).unwrap();

        let mut first = manager(&project_config(&source, &all_at_once));
        first.find_sprites();
        first.save();
        manager(&project_config(&source, &streamed)).generate();

        let expected = files(&all_at_once);
        assert_eq!(
            expected.len(),
            3 * 4,
            "png, css, json and metadata per sprite"
        );
        assert_eq!(expected, files(&streamed));
    });
}

#[test]
fn generate_releases_pixels_but_keeps_the_layout() {
    with_tmp_dir(|tmp| {
        let source = project(tmp);
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut streamed = manager(&project_config(&source, &out));
        streamed.generate();

        assert_eq!(streamed.sprites.len(), 3);
        for sprite in &streamed.sprites {
            assert!(!sprite.images.is_empty());
            let (width, height) = sprite.dimensions();
            assert!(width > 0 && height > 0, "layout must be kept");
            for img in &sprite.images {
                assert_eq!(
                    img.image.width(),
                    0,
                    "pixels of {} must be released",
                    img.path
                );
                assert!(img.computed_width > 0);
            }
        }
    });
}

#[test]
fn check_matches_is_up_to_date() {
    with_tmp_dir(|tmp| {
        let source = project(tmp);
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let config = Config {
            force: false,
            ..project_config(&source, &out)
        };

        assert!(!manager(&config).check(), "nothing generated yet");
        manager(&config).generate();
        assert!(manager(&config).check());
        let mut full = manager(&config);
        full.find_sprites();
        assert!(full.is_up_to_date());

        write_icon(&source.join("flags/icon0.png"), 999);
        assert!(!manager(&config).check(), "a changed icon must be detected");
    });
}

#[test]
fn parallel_discovery_is_deterministic_with_duplicates() {
    with_tmp_dir(|tmp| {
        let source = tmp.join("many");
        std::fs::create_dir_all(&source).unwrap();
        for index in 0..60u32 {
            write_icon(&source.join(format!("icon{index:02}.png")), index);
        }
        // Duplicates of icon07 and icon20.
        for (copy, original) in [("dup-a.png", "icon07.png"), ("dup-b.png", "icon20.png")] {
            std::fs::copy(source.join(original), source.join(copy)).unwrap();
        }
        let config = Config {
            source: source.clone(),
            output: tmp.to_path_buf(),
            ..Default::default()
        };

        let layout = || {
            let mut manager = manager(&config);
            manager.find_sprites();
            manager.sprites[0]
                .images
                .iter()
                .map(|img| (img.path.clone(), img.x, img.y, img.aliases.clone()))
                .collect::<Vec<_>>()
        };
        let first = layout();
        assert_eq!(first.len(), 60, "duplicates are consolidated");
        let aliases: usize = first.iter().map(|(_, _, _, aliases)| aliases.len()).sum();
        assert_eq!(aliases, 2);
        for _ in 0..5 {
            assert_eq!(layout(), first, "parallel reads must not change the result");
        }
    });
}

#[cfg(unix)]
#[test]
fn unreadable_file_is_skipped_and_others_are_kept() {
    use std::os::unix::fs::PermissionsExt;
    with_tmp_dir(|tmp| {
        let source = tmp.join("icons");
        std::fs::create_dir_all(&source).unwrap();
        for index in 0..4u32 {
            write_icon(&source.join(format!("icon{index}.png")), index);
        }
        let locked = source.join("icon0.png");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Running as root bypasses permissions: then every file is readable.
        let expected = if std::fs::read(&locked).is_ok() { 4 } else { 3 };

        let mut manager = manager(&Config {
            source: source.clone(),
            output: tmp.to_path_buf(),
            ..Default::default()
        });
        manager.find_sprites();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(manager.sprites[0].images.len(), expected);
    });
}
