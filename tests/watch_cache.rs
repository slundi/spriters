//! Image cache for `--watch` and `stats` pixel counting (review 5.6).

use spriters::{Algorithm, Config, ImageCache, Manager};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_watch_cache_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(&tmp);
    std::fs::remove_dir_all(&tmp).ok();
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn write_icon(path: &Path, color: [u8; 4], width: u32) {
    image::RgbaImage::from_pixel(width, 8, image::Rgba(color))
        .save(path)
        .unwrap();
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

fn set_modified(path: &Path, time: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

/// `<tmp>/icons` with `a` (red, 8×8) and `b` (blue, 8×8), plus an output directory.
fn icons(tmp: &Path) -> (PathBuf, PathBuf) {
    let source = tmp.join("icons");
    let out = tmp.join("out");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    write_icon(&source.join("a.png"), RED, 8);
    write_icon(&source.join("b.png"), BLUE, 8);
    (source, out)
}

fn config(source: &Path, out: &Path) -> Config {
    Config {
        source: source.to_path_buf(),
        output: out.to_path_buf(),
        algorithm: Algorithm::Horizontal,
        ordering: spriters::Ordering::FilenameReverse,
        force: true,
        ..Default::default()
    }
}

fn run(config: &Config, cache: Option<&Arc<ImageCache>>) -> Manager {
    let mut manager = Manager {
        project: config.project,
        config: config.clone(),
        cache: cache.cloned(),
        ..Default::default()
    };
    manager.generate();
    manager
}

/// Colour of the sprite at the top-left of the image whose file name ends with `file`.
fn color_of(manager: &Manager, out: &Path, file: &str) -> [u8; 4] {
    let img = manager.sprites[0]
        .images
        .iter()
        .find(|img| img.path.ends_with(file))
        .unwrap();
    let sprite = image::open(out.join("icons.png")).unwrap().to_rgba8();
    sprite.get_pixel(img.x, img.y).0
}

fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (name, std::fs::read(entry.path()).unwrap())
        })
        .collect()
}

#[test]
fn unchanged_files_are_not_decoded_again() {
    // Review 5.6: every --watch run re-read and re-decoded all images.
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let config = config(&source, &out);
        let cache = Arc::new(ImageCache::default());
        run(&config, Some(&cache));
        assert_eq!(cache.len(), 2);

        // Same size and modification time, different content: served from the cache.
        let file = source.join("a.png");
        let stamp = modified(&file);
        write_icon(&file, GREEN, 8);
        set_modified(&file, stamp);
        let manager = run(&config, Some(&cache));
        assert_eq!(
            color_of(&manager, &out, "a.png"),
            RED,
            "a.png must come from the cache"
        );

        // Touched: decoded again.
        set_modified(&file, stamp + Duration::from_secs(5));
        let manager = run(&config, Some(&cache));
        assert_eq!(color_of(&manager, &out, "a.png"), GREEN);
    });
}

#[test]
fn size_change_is_detected_even_with_the_same_time() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let config = config(&source, &out);
        let cache = Arc::new(ImageCache::default());
        run(&config, Some(&cache));

        let file = source.join("b.png");
        let stamp = modified(&file);
        write_icon(&file, GREEN, 12);
        set_modified(&file, stamp);
        let manager = run(&config, Some(&cache));
        let b = manager.sprites[0]
            .images
            .iter()
            .find(|img| img.path.ends_with("b.png"))
            .unwrap();
        assert_eq!(b.computed_width, 12);
    });
}

#[test]
fn processing_options_are_part_of_the_cache_key() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let cache = Arc::new(ImageCache::default());
        run(&config(&source, &out), Some(&cache));

        // The same files with --max-width: the cached full-size images must not be reused.
        let limited = Config {
            max_width: Some(4),
            ..config(&source, &out)
        };
        let manager = run(&limited, Some(&cache));
        for img in &manager.sprites[0].images {
            assert_eq!(img.computed_width, 4, "{}", img.path);
        }
    });
}

#[test]
fn cached_runs_write_the_same_files() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let config = Config {
            json: true,
            ..config(&source, &out)
        };
        run(&config, None);
        let uncached = files(&out);

        let cache = Arc::new(ImageCache::default());
        run(&config, Some(&cache));
        run(&config, Some(&cache)); // fully served from the cache
        assert_eq!(files(&out), uncached);
    });
}

#[test]
fn deleted_files_are_forgotten() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        let config = config(&source, &out);
        let cache = Arc::new(ImageCache::default());
        run(&config, Some(&cache));
        assert_eq!(cache.len(), 2);

        std::fs::remove_file(source.join("b.png")).unwrap();
        cache.retain_existing();
        assert_eq!(cache.len(), 1);
        let manager = run(&config, Some(&cache));
        assert_eq!(manager.sprites[0].images.len(), 1);
    });
}

#[test]
fn stats_count_visible_pixels() {
    with_tmp_dir(|tmp| {
        let (source, out) = icons(tmp);
        // Padding adds transparent pixels around both icons.
        let config = Config {
            padding: Some(String::from("1")),
            ..config(&source, &out)
        };
        run(&config, None);
        let stats = spriters::sprite_stats(&out, "icons").unwrap();
        assert_eq!((stats.width, stats.height), (20, 10));
        assert_eq!(stats.total_pixels, 200);
        assert_eq!(stats.used_pixels, 2 * 8 * 8);

        // Same count from an 8-bit palette sprite.
        run(
            &Config {
                png8: true,
                ..config
            },
            None,
        );
        let stats = spriters::sprite_stats(&out, "icons").unwrap();
        assert_eq!(stats.used_pixels, 2 * 8 * 8);
    });
}
