use spriters::{Config, Manager};
use std::path::PathBuf;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn with_tmp_dir<F: FnOnce(&std::path::Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_cidx_{}",
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

fn run(config: Config) -> Manager {
    let mut manager = Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
    manager
}

// ─── 4.1 Dry-run ─────────────────────────────────────────────────────────────

#[test]
fn dry_run_creates_no_files() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.dry_run = true;

        run(config);

        let entries: Vec<_> = std::fs::read_dir(out)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            entries.is_empty(),
            "dry-run must not write any files; found: {:?}",
            entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
        );
    });
}

#[test]
fn dry_run_still_discovers_images() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.dry_run = true;

        let mut manager = Manager {
            config: config.clone(),
            ..Default::default()
        };
        manager.find_sprites();
        // Images must be discovered even though nothing will be saved.
        assert!(
            !manager.sprites.is_empty(),
            "dry-run must still discover sprites"
        );
        assert!(
            !manager.sprites[0].images.is_empty(),
            "dry-run must still load images"
        );
    });
}

// ─── 4.3 Check mode (via Manager::is_up_to_date) ────────────────────────────

#[test]
fn check_stale_before_first_generation() {
    with_tmp_dir(|out| {
        let config = base_config("tests/images/america", out);

        let mut manager = Manager {
            config: config.clone(),
            ..Default::default()
        };
        manager.find_sprites();

        // Nothing has been written yet — sprite must be considered stale.
        assert!(
            !manager.is_up_to_date(),
            "sprite must be stale before first generation"
        );
    });
}

#[test]
fn check_up_to_date_after_generation() {
    with_tmp_dir(|out| {
        // Generate with force=true (base_config default) so the files are written.
        run(base_config("tests/images/america", out));

        // Check freshness WITHOUT force — force bypasses the hash comparison.
        let mut check_config = base_config("tests/images/america", out);
        check_config.force = false;

        let mut manager = Manager {
            config: check_config,
            ..Default::default()
        };
        manager.find_sprites();

        assert!(
            manager.is_up_to_date(),
            "sprite must be up to date immediately after generation"
        );
    });
}

#[test]
fn force_flag_marks_sprite_as_requiring_regeneration() {
    with_tmp_dir(|out| {
        // Generate first.
        run(base_config("tests/images/america", out));

        // With --force, should_regenerate always returns true, so is_up_to_date is false.
        let mut config = base_config("tests/images/america", out);
        config.force = true;

        let mut manager = Manager {
            config: config.clone(),
            ..Default::default()
        };
        manager.find_sprites();

        assert!(
            !manager.is_up_to_date(),
            "--force must always report the sprite as requiring regeneration"
        );
    });
}

// ─── Content-based regeneration hash ─────────────────────────────────────────

fn write_solid_png(path: &std::path::Path, color: [u8; 4]) {
    image::RgbaImage::from_pixel(4, 4, image::Rgba(color))
        .save(path)
        .unwrap();
}

/// Build a source folder `<root>/icons` containing two distinct 4×4 images.
fn make_icons_source(root: &std::path::Path) -> PathBuf {
    let source = root.join("icons");
    std::fs::create_dir_all(&source).unwrap();
    write_solid_png(&source.join("a.png"), [255, 0, 0, 255]);
    write_solid_png(&source.join("b.png"), [0, 0, 255, 255]);
    source
}

fn is_up_to_date(config: &Config) -> bool {
    let mut manager = Manager {
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.is_up_to_date()
}

#[test]
fn pixel_change_with_same_size_triggers_regeneration() {
    with_tmp_dir(|tmp| {
        let source = make_icons_source(tmp);
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut config = base_config(source.to_str().unwrap(), &out);
        config.force = false;
        config.cachebuster = true;

        run(config.clone());
        assert!(is_up_to_date(&config));
        let first_sprite = std::fs::read(out.join("icons.png")).unwrap();
        let first_css = std::fs::read_to_string(out.join("icons.css")).unwrap();

        // Same dimensions, different colour.
        write_solid_png(&source.join("a.png"), [0, 255, 0, 255]);
        assert!(
            !is_up_to_date(&config),
            "changing pixels without changing size must invalidate the sprite"
        );

        run(config.clone());
        let second_sprite = std::fs::read(out.join("icons.png")).unwrap();
        let second_css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert_ne!(first_sprite, second_sprite, "sprite must be regenerated");
        assert_ne!(first_css, second_css, "cachebuster must change");
        assert!(is_up_to_date(&config));
    });
}

#[test]
fn resize_algorithm_change_triggers_regeneration() {
    with_tmp_dir(|tmp| {
        let source = make_icons_source(tmp);
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut config = base_config(source.to_str().unwrap(), &out);
        config.force = false;

        run(config.clone());
        assert!(is_up_to_date(&config));

        config.resize_algorithm = image::imageops::FilterType::Nearest;
        assert!(
            !is_up_to_date(&config),
            "changing the resize filter must invalidate the sprite"
        );
    });
}

#[test]
fn algorithm_change_triggers_regeneration() {
    with_tmp_dir(|tmp| {
        let source = tmp.join("icons");
        std::fs::create_dir_all(&source).unwrap();
        // A single image has the same position under every layout algorithm.
        write_solid_png(&source.join("a.png"), [255, 0, 0, 255]);
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut config = base_config(source.to_str().unwrap(), &out);
        config.force = false;
        config.algorithm = spriters::Algorithm::Vertical;

        run(config.clone());
        assert!(is_up_to_date(&config));

        config.algorithm = spriters::Algorithm::Horizontal;
        assert!(
            !is_up_to_date(&config),
            "changing the layout algorithm must invalidate the sprite"
        );
    });
}

// ─── Text outputs on an up-to-date sprite ────────────────────────────────────

fn mtime(path: &std::path::Path) -> std::time::SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

/// Generate `<tmp>/icons` into `<tmp>/out` once and return a non-forced config for it.
fn generated_icons(tmp: &std::path::Path) -> (Config, PathBuf) {
    let source = make_icons_source(tmp);
    let out = tmp.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut config = base_config(source.to_str().unwrap(), &out);
    config.force = false;
    run(config.clone());
    assert!(is_up_to_date(&config));
    (config, out)
}

#[test]
fn adding_output_flag_on_up_to_date_sprite_creates_file() {
    with_tmp_dir(|tmp| {
        let (mut config, out) = generated_icons(tmp);

        config.json = true;
        assert!(!is_up_to_date(&config), "a missing output must be stale");
        run(config.clone());

        assert!(out.join("icons.json").exists(), "JSON must be generated");
        assert!(is_up_to_date(&config));
    });
}

#[test]
fn namespace_change_rewrites_css() {
    with_tmp_dir(|tmp| {
        let (mut config, out) = generated_icons(tmp);

        config.namespace = Some(String::from("brandnew"));
        assert!(!is_up_to_date(&config));
        run(config.clone());

        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert!(css.contains("brandnew"), "CSS must use the new namespace");
        assert!(is_up_to_date(&config));
    });
}

#[test]
fn url_change_rewrites_css() {
    with_tmp_dir(|tmp| {
        let (mut config, out) = generated_icons(tmp);

        config.url = Some(String::from("https://cdn.example.com/sprites/"));
        assert!(!is_up_to_date(&config));
        run(config.clone());

        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert!(
            css.contains("https://cdn.example.com/sprites/"),
            "CSS must use the new URL"
        );
    });
}

#[test]
fn deleted_output_is_regenerated() {
    with_tmp_dir(|tmp| {
        let (config, out) = generated_icons(tmp);

        std::fs::remove_file(out.join("icons.css")).unwrap();
        assert!(!is_up_to_date(&config));
        run(config.clone());

        assert!(out.join("icons.css").exists());
    });
}

#[test]
fn text_only_change_does_not_rewrite_image() {
    with_tmp_dir(|tmp| {
        let (mut config, out) = generated_icons(tmp);
        let image_mtime = mtime(&out.join("icons.png"));

        config.namespace = Some(String::from("other"));
        run(config.clone());

        assert_eq!(
            image_mtime,
            mtime(&out.join("icons.png")),
            "a text-only option must not regenerate the image"
        );
    });
}

#[test]
fn unchanged_run_rewrites_nothing() {
    with_tmp_dir(|tmp| {
        let (config, out) = generated_icons(tmp);
        let image_mtime = mtime(&out.join("icons.png"));
        let css_mtime = mtime(&out.join("icons.css"));

        run(config.clone());

        assert_eq!(image_mtime, mtime(&out.join("icons.png")));
        assert_eq!(css_mtime, mtime(&out.join("icons.css")));
    });
}

#[test]
fn every_output_format_is_up_to_date_after_generation() {
    with_tmp_dir(|tmp| {
        let source = make_icons_source(tmp);
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut config = base_config(source.to_str().unwrap(), &out);
        config.force = false;
        config.html = true;
        config.less = true;
        config.scss = true;
        config.json = true;
        config.cocos2d = true;
        config.caat = true;
        config.css_vars = true;
        config.tailwind = true;
        config.dts = true;

        run(config.clone());

        // Fails if an expected file name drifts from the one a generator writes.
        assert!(is_up_to_date(&config));
    });
}

#[test]
fn no_img_run_then_full_run_generates_image() {
    with_tmp_dir(|tmp| {
        let source = make_icons_source(tmp);
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut config = base_config(source.to_str().unwrap(), &out);
        config.force = false;
        config.no_img = true;

        run(config.clone());
        assert!(out.join("icons.css").exists());
        assert!(!out.join("icons.png").exists());

        config.no_img = false;
        assert!(!is_up_to_date(&config), "the image is still missing");
        run(config.clone());
        assert!(out.join("icons.png").exists());
        assert!(is_up_to_date(&config));
    });
}

// ─── Cache-busted sprite file names ──────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum Buster {
    None,
    Query,
    Filename,
    FilenameOnlySprites,
}
const BUSTERS: [Buster; 4] = [
    Buster::None,
    Buster::Query,
    Buster::Filename,
    Buster::FilenameOnlySprites,
];

fn with_buster(mut config: Config, buster: Buster) -> Config {
    match buster {
        Buster::None => {}
        Buster::Query => config.cachebuster = true,
        Buster::Filename => config.cachebuster_filename = true,
        Buster::FilenameOnlySprites => config.cachebuster_filename_only_sprites = true,
    }
    config
}

/// Every `url('...')` in a stylesheet, without query string.
fn css_urls(css: &str) -> Vec<String> {
    css.split("url('")
        .skip(1)
        .map(|rest| {
            let url = &rest[..rest.find('\'').unwrap()];
            url.split('?').next().unwrap().to_owned()
        })
        .collect()
}

/// Value of the first `"key": "value"` pair in a JSON document.
fn json_string(json: &str, key: &str) -> String {
    let start = json.find(&format!("\"{key}\": \"")).unwrap() + key.len() + 5;
    json[start..start + json[start..].find('"').unwrap()].to_owned()
}

/// Sprite images (`icons.png`, `icons_<hash>.png`) present in `out`.
fn sprite_images(out: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(out)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("icons") && name.ends_with(".png"))
        .collect();
    names.sort();
    names
}

fn icons_config(tmp: &std::path::Path, buster: Buster) -> (Config, PathBuf, PathBuf) {
    let source = make_icons_source(tmp);
    let out = tmp.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut config = base_config(source.to_str().unwrap(), &out);
    config.force = false;
    config.json = true;
    config.caat = true;
    (with_buster(config, buster), source, out)
}

#[test]
fn every_cachebuster_references_an_existing_sprite() {
    for buster in BUSTERS {
        with_tmp_dir(|tmp| {
            let (config, _, out) = icons_config(tmp, buster);
            run(config);

            let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
            let urls = css_urls(&css);
            assert!(
                !urls.is_empty(),
                "[{buster:?}] CSS must reference the sprite"
            );
            for url in urls {
                assert!(
                    out.join(&url).exists(),
                    "[{buster:?}] CSS references missing {url}; found {:?}",
                    sprite_images(&out)
                );
            }

            let json = std::fs::read_to_string(out.join("icons.json")).unwrap();
            let json_url = json_string(&json, "sprite_url");
            let json_file = json_url.split('?').next().unwrap();
            assert!(
                out.join(json_file).exists(),
                "[{buster:?}] JSON references missing {json_url}"
            );

            let caat = std::fs::read_to_string(out.join("icons.caat.json")).unwrap();
            let caat_file = json_string(&caat, "sprite_filename");
            assert!(
                out.join(&caat_file).exists(),
                "[{buster:?}] CAAT references missing {caat_file}"
            );

            assert_eq!(
                sprite_images(&out).len(),
                1,
                "[{buster:?}] exactly one sprite image must be written"
            );
        });
    }
}

#[test]
fn query_cachebuster_keeps_plain_filename() {
    with_tmp_dir(|tmp| {
        let (config, _, out) = icons_config(tmp, Buster::Query);
        run(config);

        assert_eq!(sprite_images(&out), vec!["icons.png"]);
        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert!(
            css.contains("url('icons.png?"),
            "query buster expected: {css}"
        );
    });
}

#[test]
fn hashed_filename_sprite_is_up_to_date_on_second_run() {
    for buster in [Buster::Filename, Buster::FilenameOnlySprites] {
        with_tmp_dir(|tmp| {
            let (config, _, out) = icons_config(tmp, buster);
            run(config.clone());
            let image = out.join(&sprite_images(&out)[0]);
            let image_mtime = mtime(&image);

            assert!(is_up_to_date(&config), "[{buster:?}] must be up to date");
            run(config);
            assert_eq!(
                image_mtime,
                mtime(&image),
                "[{buster:?}] sprite must not be regenerated"
            );
        });
    }
}

#[test]
fn hashed_filename_sprite_replaces_previous_one() {
    for buster in [Buster::Filename, Buster::FilenameOnlySprites] {
        with_tmp_dir(|tmp| {
            let (config, source, out) = icons_config(tmp, buster);
            run(config.clone());
            let first = sprite_images(&out);

            write_solid_png(&source.join("a.png"), [0, 255, 0, 255]);
            run(config);
            let second = sprite_images(&out);

            assert_eq!(second.len(), 1, "[{buster:?}] old sprite must be removed");
            assert_ne!(first, second, "[{buster:?}] hash must change");
            let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
            assert_eq!(css_urls(&css)[0], second[0]);
        });
    }
}

#[test]
fn stale_sprite_cleanup_keeps_unrelated_files() {
    with_tmp_dir(|tmp| {
        let (config, source, out) = icons_config(tmp, Buster::Filename);
        let unrelated = [
            "icons_notahash.png",
            "icons_1234567.png",
            "icons_12345678.webp",
            "other_12345678.png",
            "icons.png",
        ];
        for name in unrelated {
            std::fs::write(out.join(name), b"keep").unwrap();
        }

        run(config.clone());
        write_solid_png(&source.join("a.png"), [0, 255, 0, 255]);
        run(config);

        for name in unrelated {
            assert!(out.join(name).exists(), "{name} must not be removed");
        }
    });
}

#[test]
fn sprite_stats_finds_hashed_sprite() {
    with_tmp_dir(|tmp| {
        let (config, _, out) = icons_config(tmp, Buster::FilenameOnlySprites);
        run(config);

        let stats =
            spriters::sprite_stats(&out, "icons").expect("stats must find the cache-busted sprite");
        assert!(stats.width > 0 && stats.height > 0);
    });
}

// ─── Crop of blank images ────────────────────────────────────────────────────

#[test]
fn crop_keeps_blank_icon_as_one_pixel_placeholder() {
    with_tmp_dir(|tmp| {
        let source = make_icons_source(tmp);
        image::RgbaImage::new(16, 16)
            .save(source.join("blank.png"))
            .unwrap();
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut config = base_config(source.to_str().unwrap(), &out);
        config.crop = true;

        let manager = run(config);

        let blank = manager.sprites[0]
            .images
            .iter()
            .find(|img| img.path.ends_with("blank.png"))
            .expect("blank icon must stay in the sprite");
        assert_eq!((blank.computed_width, blank.computed_height), (1, 1));

        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert!(css.contains("blank"), "blank icon must keep a CSS rule");
        assert!(!css.contains("width: 0px"), "no zero-sized rule: {css}");
        assert!(!css.contains("height: 0px"), "no zero-sized rule: {css}");
    });
}

// ─── 4.4 Stats ───────────────────────────────────────────────────────────────

#[test]
fn list_sprite_names_finds_generated_sprites() {
    with_tmp_dir(|out| {
        run(base_config("tests/images/america", out));

        let names = spriters::list_sprite_names(out);
        assert_eq!(names, vec!["america"]);
    });
}

#[test]
fn list_sprite_names_empty_for_fresh_dir() {
    with_tmp_dir(|out| {
        assert!(spriters::list_sprite_names(out).is_empty());
    });
}

#[test]
fn sprite_stats_dimensions_and_efficiency() {
    with_tmp_dir(|out| {
        run(base_config("tests/images/america", out));

        let stats = spriters::sprite_stats(out, "america")
            .expect("stats must be available after generation");

        assert!(stats.width > 0);
        assert!(stats.height > 0);
        assert!(stats.file_size > 0);
        assert!(stats.total_pixels > 0);
        assert!(stats.used_pixels <= stats.total_pixels);
        let pct = stats.packing_efficiency_pct();
        assert!((1..=100).contains(&pct), "efficiency {pct}% must be 1–100");
    });
}

#[test]
fn sprite_stats_fails_for_missing_sprite() {
    with_tmp_dir(|out| {
        let error = spriters::sprite_stats(out, "nonexistent").unwrap_err();
        assert!(matches!(error, spriters::StatsError::NotFound), "{error}");
    });
}

// `stats` looked for name.png, then .webp, then .avif.

#[test]
fn sprite_stats_reads_the_recorded_format_not_a_stale_png() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.sprite_format = spriters::SpriteFormat::WebP;
        run(config);
        // Left by a run before the switch to WebP, and newer than the WebP sprite.
        image::RgbaImage::new(3, 3)
            .save(out.join("america.png"))
            .unwrap();

        let stats = spriters::sprite_stats(out, "america").unwrap();
        let webp = image::open(out.join("america.webp")).unwrap();
        assert_eq!((stats.width, stats.height), (webp.width(), webp.height()));
        assert_eq!(
            stats.file_size,
            std::fs::metadata(out.join("america.webp")).unwrap().len()
        );
    });
}

#[test]
fn sprite_stats_reports_a_missing_recorded_image() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.sprite_format = spriters::SpriteFormat::WebP;
        run(config);
        std::fs::remove_file(out.join("america.webp")).unwrap();
        image::RgbaImage::new(3, 3)
            .save(out.join("america.png"))
            .unwrap();

        // The stale PNG is not taken for the sprite.
        let error = spriters::sprite_stats(out, "america").unwrap_err();
        assert!(
            matches!(&error, spriters::StatsError::Missing(path) if path.ends_with("america.webp")),
            "{error}"
        );
    });
}

#[test]
fn sprite_stats_without_recorded_image_takes_the_newest_sprite() {
    with_tmp_dir(|out| {
        // Metadata of an older version: hashes only, no image file name.
        std::fs::write(out.join(".icons.spriters"), "1\n2\n").unwrap();
        let save = |file: &str, size: u32| {
            image::RgbaImage::new(size, size)
                .save(out.join(file))
                .unwrap();
            // Distinct modification times, even on coarse file systems.
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        save("icons.png", 2);
        save("icons_0123abcd.webp", 3);
        // Not the default image of the sprite: another ratio, another sprite.
        save("icons_0123abcd@2x.webp", 6);
        save("icons2.png", 7);

        let stats = spriters::sprite_stats(out, "icons").unwrap();
        assert_eq!((stats.width, stats.height), (3, 3));
    });
}

// ─── Sprite names with a dot ─────────────────────────────────────────────────

#[test]
fn dotted_directory_names_the_sprite_in_full() {
    // In project mode the name came from `file_stem()` (`icons`) while the
    // outputs used `file_name()` (`icons.v2.css`).
    with_tmp_dir(|tmp| {
        let project = tmp.join("project");
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).unwrap();
        for (dir, color) in [("icons.v2", [255, 0, 0, 255]), ("flags", [0, 0, 255, 255])] {
            std::fs::create_dir_all(project.join(dir)).unwrap();
            write_solid_png(&project.join(dir).join("a.png"), color);
        }
        let config = Config {
            source: project.clone(),
            output: out.clone(),
            project: true,
            json: true,
            ..Default::default()
        };
        let mut manager = Manager {
            project: true,
            config: config.clone(),
            ..Default::default()
        };
        assert!(manager.generate());

        let mut names: Vec<String> = manager.sprites.iter().map(|sprite| sprite.name()).collect();
        names.sort();
        assert_eq!(names, ["flags", "icons.v2"]);
        for file in [
            "icons.v2.png",
            "icons.v2.css",
            "icons.v2.json",
            ".icons.v2.spriters",
        ] {
            assert!(out.join(file).is_file(), "{file}");
        }
        assert!(!out.join("icons.png").exists() && !out.join("icons.css").exists());
        let css = std::fs::read_to_string(out.join("icons.v2.css")).unwrap();
        assert!(css.contains("url('icons.v2.png')"), "{css}");

        // Every reader uses the same name: metadata, stats, staleness.
        let mut listed = spriters::list_sprite_names(&out);
        listed.sort();
        assert_eq!(listed, ["flags", "icons.v2"]);
        assert!(spriters::sprite_stats(&out, "icons.v2").is_ok());
        let mut again = Manager {
            project: true,
            config,
            ..Default::default()
        };
        assert!(
            again.check(),
            "the second run must find the first one's outputs"
        );
    });
}

#[test]
fn dotted_names_do_not_claim_other_sprites_files() {
    // `icons.v2.css` belongs to the sprite `icons.v2`, not to a sprite `icons`.
    with_tmp_dir(|tmp| {
        let source = tmp.join("icons");
        std::fs::create_dir_all(&source).unwrap();
        write_solid_png(&source.join("a.png"), [255, 0, 0, 255]);
        let config = Config {
            source: source.clone(),
            output: source.clone(),
            ..Default::default()
        };
        let filter = spriters::OutputFilter::new(&config);
        assert!(filter.is_generated_file(&source.join("icons.css")));
        for file in ["icons.v2.css", "icons.v2.png", ".icons.v2.spriters"] {
            assert!(!filter.is_generated_file(&source.join(file)), "{file}");
        }
    });
}

// ─── 4.5 Shell completions ───────────────────────────────────────────────────

#[test]
fn bash_completion_covers_key_flags() {
    let script = spriters::completions::BASH;
    for flag in &[
        "--dry-run",
        "--check",
        "--exclude",
        "--css-vars",
        "--format",
        "--algorithm",
        "--stdin",
    ] {
        assert!(script.contains(flag), "bash completion missing flag {flag}");
    }
    assert!(
        script.contains("complete -F _spriters spriters"),
        "must register completion"
    );
}

#[test]
fn zsh_completion_covers_key_flags() {
    let script = spriters::completions::ZSH;
    for flag in &["--dry-run", "--check", "--exclude", "--css-vars", "--stdin"] {
        assert!(script.contains(flag), "zsh completion missing flag {flag}");
    }
    assert!(
        script.contains("#compdef spriters"),
        "must start with #compdef"
    );
}

#[test]
fn fish_completion_covers_key_flags() {
    let script = spriters::completions::FISH;
    for flag in &[
        "dry-run",
        "check",
        "exclude",
        "css-vars",
        "algorithm",
        "stdin",
    ] {
        assert!(script.contains(flag), "fish completion missing flag {flag}");
    }
    assert!(
        script.contains("complete -c spriters"),
        "must use complete -c spriters"
    );
}
