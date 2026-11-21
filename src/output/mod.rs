//! Generation of the sprite files: images, text outputs and the metadata of the run.

mod classes;
mod css;
mod data;
mod files;
mod html;
mod meta;
mod names;
mod png8;
mod render;
mod template;

use std::io::Write;

use crate::{Config, Sprite};
use files::write_output;
use meta::{compute_outputs_hash, compute_sprite_hash, read_metadata, save_metadata};
use names::{sprite_filename, sprite_filenames};
use template::templated;

pub(crate) use meta::{read_metadata_file, staleness};
pub(crate) use names::{is_generated_file_name, meta_file_name};
pub(crate) use render::find_sprite_image;
pub(crate) use template::check_template;

const SPRITERS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Writes the content of a text output; every write error is returned.
type TextWriter = fn(&Config, &Sprite, &mut dyn Write) -> std::io::Result<()>;

/// A text output: file suffix (after `<sprite name>.`), name used in logs, and writer.
struct TextOutput {
    suffix: &'static str,
    kind: &'static str,
    write: TextWriter,
}

/// The text outputs enabled in `config`.
fn text_outputs(config: &Config) -> Vec<TextOutput> {
    let all: [(bool, &'static str, &'static str, TextWriter); 10] = [
        (!config.no_css, "css", "CSS", |config, sprite, f| {
            templated(&config.css_template, css::generate_css, config, sprite, f)
        }),
        // The built-in LESS output is the CSS one.
        (config.less, "less", "LESS", |config, sprite, f| {
            templated(&config.less_template, css::generate_css, config, sprite, f)
        }),
        (config.scss, "scss", "SCSS", |config, sprite, f| {
            templated(&config.scss_template, css::generate_scss, config, sprite, f)
        }),
        (config.json, "json", "JSON", data::generate_json),
        (config.html, "html", "HTML", html::generate_html),
        (
            config.cocos2d,
            "plist",
            "Cocos2d plist",
            data::generate_coco2d,
        ),
        (config.caat, "caat.json", "CAAT", data::generate_caat),
        (
            config.css_vars,
            "vars.css",
            "CSS vars",
            |config, sprite, f| {
                templated(
                    &config.css_vars_template,
                    css::generate_css_vars,
                    config,
                    sprite,
                    f,
                )
            },
        ),
        (
            config.tailwind,
            "tailwind.json",
            "Tailwind JSON",
            data::generate_tailwind,
        ),
        (
            config.dts,
            "d.ts",
            "TypeScript definitions",
            data::generate_dts,
        ),
    ];
    all.into_iter()
        .filter(|(enabled, ..)| *enabled)
        .map(|(_, suffix, kind, write)| TextOutput {
            suffix,
            kind,
            write,
        })
        .collect()
}

/// File names of the text outputs enabled in `config`.
fn text_output_names(config: &Config, name: &str) -> Vec<String> {
    text_outputs(config)
        .iter()
        .map(|output| format!("{name}.{}", output.suffix))
        .collect()
}

/// What `--dry-run` prints for `sprite`: its classes and the files that would be written.
fn dry_run_report(config: &Config, sprite: &Sprite) -> String {
    let name = sprite.name();
    let (w, h) = sprite.dimensions();
    let total_entries: usize = sprite.images.iter().map(|i| 1 + i.aliases.len()).sum();

    let mut lines = vec![format!(
        "Sprite: {} ({}×{} px, {} entries)",
        name, w, h, total_entries
    )];
    for entry in classes::entries(config, sprite) {
        let img = entry.image;
        lines.push(format!(
            "  {selector:41} at ({x},{y}) {w}×{h}px",
            selector = entry.selector(),
            x = img.x,
            y = img.y,
            w = img.computed_width,
            h = img.computed_height,
        ));
    }
    let images = if config.no_img {
        Vec::new()
    } else {
        sprite_filenames(config, sprite)
    };
    let texts = text_output_names(config, &name);
    let mut groups = vec![(config.image_dir(), images), (config.css_dir(), texts)];
    if config.image_dir() == config.css_dir() {
        let texts = groups.pop().map(|(_, texts)| texts).unwrap_or_default();
        groups[0].1.extend(texts);
    } else {
        groups.retain(|(_, files)| !files.is_empty());
    }
    for (dir, files) in groups {
        lines.push(format!("Would write to {}:", dir.display()));
        lines.extend(files.into_iter().map(|file| format!("  {file}")));
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Print the dry-run report of `sprite` in a single write under the stdout lock: in project
/// mode sprites are processed in parallel, and their reports must not interleave.
fn print_dry_run(config: &Config, sprite: &Sprite) -> std::io::Result<()> {
    let report = dry_run_report(config, sprite);
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(report.as_bytes())
        .and_then(|()| stdout.flush())
        .inspect_err(|e| log::error!("Cannot print the dry run: {e}"))
}

/// Write the sprite image and the text outputs that are out of date. Returns whether every
/// file that had to be written was written.
pub fn generate_files(config: &Config, sprite: &Sprite) -> bool {
    if config.dry_run {
        return print_dry_run(config, sprite).is_ok();
    }

    let stored = read_metadata(config, sprite);
    let stale = meta::staleness_from(config, sprite, &stored);
    if !stale.any() {
        log::info!("Sprite is up to date, skipping generation");
        return true;
    }

    let image_written = !stale.image || render::generate_sprite_image(config, sprite);
    let outputs_written = !stale.outputs || generate_text_outputs(config, sprite);

    // With --no-img the image on disk is untouched, and a failed write leaves the previous
    // image intact (see `replace_file`): keep what was previously stored. The hash then
    // differs from the current one, so the next run tries again.
    let (image_hash, image_file) = if config.no_img || !image_written {
        (stored.image_hash, stored.image_file)
    } else {
        (
            Some(compute_sprite_hash(config, sprite)),
            Some(sprite_filename(config, sprite)),
        )
    };
    let metadata_written = save_metadata(
        config,
        sprite,
        image_hash,
        image_file.as_deref(),
        outputs_written.then(|| compute_outputs_hash(config, sprite)),
    )
    .is_ok();
    image_written && outputs_written && metadata_written
}

/// Write every enabled text output. Returns whether all of them were written.
fn generate_text_outputs(config: &Config, sprite: &Sprite) -> bool {
    use rayon::prelude::*;

    let name = sprite.name();
    // Text-format outputs are all independent — generate them in parallel.
    // Counted rather than `all`: a failure must not stop the other outputs.
    let failures = text_outputs(config)
        .par_iter()
        .filter(|output| {
            let path = config.css_dir().join(format!("{name}.{}", output.suffix));
            write_output(&path, output.kind, |f| (output.write)(config, sprite, f)).is_err()
        })
        .count();
    failures == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Accepts `remaining` bytes, then fails every write.
    struct LimitedWriter {
        remaining: usize,
    }
    impl Write for LimitedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.remaining == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::StorageFull));
            }
            let written = buf.len().min(self.remaining);
            self.remaining -= written;
            Ok(written)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Sprite with an alias and two ratios, so that every branch of the writers produces
    /// text (aliases, media queries).
    fn sprite_for_writers(config: &Config) -> Sprite {
        let image = |path: &str, x: u32, aliases: Vec<String>| crate::ImageAndPosition {
            path: String::from(path),
            image: image::DynamicImage::new_rgba8(8, 8),
            computed_width: 4,
            computed_height: 4,
            x,
            y: 0,
            aliases,
            source_hash: 0,
        };
        Sprite {
            config: config.clone(),
            path: PathBuf::from("icons"),
            ratios: vec![1.0, 2.0],
            images: vec![
                image("icons/red.png", 0, vec![String::from("icons/copy.png")]),
                image("icons/blue.png", 4, vec![]),
            ],
            ..Default::default()
        }
    }

    fn all_text_outputs() -> Config {
        Config {
            html: true,
            less: true,
            scss: true,
            cocos2d: true,
            json: true,
            caat: true,
            css_vars: true,
            tailwind: true,
            dts: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_text_writers_report_every_write_error() {
        // Review 6.2: write errors were ignored (`let _ = writeln!`), so a full disk left
        // truncated files behind without any message. Failing at each byte offset checks
        // that no write of any writer is ignored.
        let config = all_text_outputs();
        let sprite = sprite_for_writers(&config);
        let outputs = text_outputs(&config);
        assert_eq!(outputs.len(), 10);
        for output in outputs {
            let mut full = Vec::new();
            (output.write)(&config, &sprite, &mut full).unwrap();
            assert!(!full.is_empty(), "{}", output.kind);
            for remaining in 0..full.len() {
                let result = (output.write)(&config, &sprite, &mut LimitedWriter { remaining });
                assert!(
                    result.is_err(),
                    "{}: error after {remaining} of {} bytes was ignored",
                    output.kind,
                    full.len()
                );
            }
            let mut exact = LimitedWriter {
                remaining: full.len(),
            };
            assert!((output.write)(&config, &sprite, &mut exact).is_ok());
        }
    }

    #[test]
    fn test_text_output_names_follow_enabled_outputs() {
        assert_eq!(
            text_output_names(&Config::default(), "icons"),
            vec!["icons.css"]
        );
        let config = Config {
            no_css: true,
            json: true,
            dts: true,
            ..Default::default()
        };
        assert_eq!(
            text_output_names(&config, "icons"),
            vec!["icons.json", "icons.d.ts"]
        );
        assert_eq!(text_output_names(&all_text_outputs(), "icons").len(), 10);
    }

    #[test]
    fn test_failed_image_keeps_previous_image_and_metadata() {
        use files::temp_path;

        let dir = std::env::temp_dir().join(format!("spriters_keep_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = Config {
            output: dir.clone(),
            force: true,
            ..Default::default()
        };
        let sprite = sprite_for_writers(&config);
        assert!(generate_files(&config, &sprite));
        let retina = dir.join("icons@2x.png");
        let previous = std::fs::read(&retina).unwrap();
        let stored = read_metadata(&config, &sprite);
        assert_eq!(
            stored.image_hash,
            Some(compute_sprite_hash(&config, &sprite))
        );

        // Another layout, whose @2x image cannot be written: its temporary file is taken.
        let mut moved = sprite_for_writers(&config);
        moved.images[1].x = 8;
        std::fs::create_dir(temp_path(&retina)).unwrap();
        assert!(!generate_files(&config, &moved));
        std::fs::remove_dir(temp_path(&retina)).unwrap();

        // The stale-sprite cleanup must not remove the image that failed to be replaced.
        assert_eq!(std::fs::read(&retina).unwrap(), previous);
        // The image on disk is still described by the previous hash, so the next run
        // regenerates it.
        let after = read_metadata(&config, &moved);
        assert_eq!(after.image_hash, stored.image_hash);
        assert_eq!(after.image_file.as_deref(), Some("icons.png"));
        assert!(
            meta::staleness_from(
                &Config {
                    force: false,
                    ..config.clone()
                },
                &moved,
                &after
            )
            .image
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_dry_run_report() {
        let config = Config {
            output: PathBuf::from("out"),
            json: true,
            ..Default::default()
        };
        let sprite = sprite_for_writers(&config);
        let class = |name: &str| format!("  .{:40}", format!("sprite-icons-{name}"));
        let expected = [
            String::from("Sprite: icons (8×4 px, 3 entries)"),
            format!("{} at (0,0) 4×4px", class("red")),
            format!("{} at (0,0) 4×4px", class("copy")),
            format!("{} at (4,0) 4×4px", class("blue")),
            String::from("Would write to out:"),
            String::from("  icons.png"),
            String::from("  icons@2x.png"),
            String::from("  icons.css"),
            String::from("  icons.json"),
            String::new(),
        ]
        .join("\n");
        assert_eq!(dry_run_report(&config, &sprite), expected);

        // Without images, only the text outputs would be written.
        let config = Config {
            no_img: true,
            ..config
        };
        let report = dry_run_report(&config, &sprite);
        assert!(report.ends_with("Would write to out:\n  icons.css\n  icons.json\n"));
    }
}
