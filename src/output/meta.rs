//! Change detection: hashes of the inputs, metadata file of the previous run, and what is
//! out of date.

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use super::files::replace_file;
use super::names::{meta_file_name, sprite_filenames};
use super::{SPRITERS_VERSION, text_output_names};
use crate::{Config, Sprite};

/// Get the path to the metadata file for a sprite
fn get_meta_path(config: &Config, sprite: &Sprite) -> PathBuf {
    let name = sprite.name();
    config.image_dir().join(meta_file_name(&name))
}

/// Compute a hash of all inputs that affect sprite generation
pub(super) fn compute_sprite_hash(config: &Config, sprite: &Sprite) -> u64 {
    use std::hash::{Hash, Hasher};
    use twox_hash::XxHash64;

    let mut hasher = XxHash64::with_seed(0);

    // Hash version
    SPRITERS_VERSION.hash(&mut hasher);

    // Hash relevant config options
    config.algorithm.hash(&mut hasher);
    config.resize_algorithm.hash(&mut hasher);
    config.sprite_format.extension().hash(&mut hasher);
    config.png8.hash(&mut hasher);
    config.crop.hash(&mut hasher);
    config.max_width.hash(&mut hasher);
    config.max_height.hash(&mut hasher);
    config.padding.hash(&mut hasher);
    config.margin.hash(&mut hasher);
    config.ratios.hash(&mut hasher);

    // Images and layout: the costly part, computed once per sprite.
    sprite
        .layout_hash
        .get_or_init(|| layout_hash(sprite))
        .hash(&mut hasher);

    hasher.finish()
}

/// Hash of each source image path, content, layout position, dimensions, and aliases.
fn layout_hash(sprite: &Sprite) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = twox_hash::XxHash64::with_seed(0);
    for img in &sprite.images {
        img.path.hash(&mut hasher);
        img.source_hash.hash(&mut hasher);
        img.x.hash(&mut hasher);
        img.y.hash(&mut hasher);
        img.computed_width.hash(&mut hasher);
        img.computed_height.hash(&mut hasher);
        for alias in &img.aliases {
            alias.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Compute a hash of all inputs that affect the text outputs (CSS, JSON, LESS, ...).
///
/// It extends [`compute_sprite_hash`] (positions and cachebuster come from the image)
/// with every option that only changes the generated text.
pub(super) fn compute_outputs_hash(config: &Config, sprite: &Sprite) -> u64 {
    use std::hash::{Hash, Hasher};
    use twox_hash::XxHash64;

    // Exhaustive on purpose: adding a field to `Config` must be a compile error here
    // until it is classified, otherwise a new option could leave stale outputs behind.
    let Config {
        // Covered by `compute_sprite_hash`, directly or through image paths/positions.
        algorithm: _,
        ordering: _,
        crop: _,
        max_width: _,
        max_height: _,
        resize_algorithm: _,
        margin: _,
        padding: _,
        ratios: _,
        png8: _,
        sprite_format: _,
        recursive: _,
        follow_links: _,
        exclude: _,
        files: _,
        source: _,
        project: _,
        no_duplicate: _,
        // No effect on generated content.
        force: _,
        quiet: _,
        output: _,
        no_img: _,
        watch: _,
        dry_run: _,
        check: _,
        // Text outputs only.
        cachebuster,
        cachebuster_filename,
        cachebuster_filename_only_sprites,
        namespace,
        sprite_namespace,
        separator,
        pseudo_class_separator,
        url,
        css,
        img,
        css_template,
        less_template,
        scss_template,
        css_vars_template,
        no_css,
        html,
        less,
        scss,
        cocos2d,
        json,
        json_format,
        caat,
        css_vars,
        tailwind,
        dts,
    } = config;

    let mut hasher = XxHash64::with_seed(0);
    compute_sprite_hash(config, sprite).hash(&mut hasher);
    (
        cachebuster,
        cachebuster_filename,
        cachebuster_filename_only_sprites,
    )
        .hash(&mut hasher);
    (
        namespace,
        sprite_namespace,
        separator,
        pseudo_class_separator,
    )
        .hash(&mut hasher);
    (url, css, img).hash(&mut hasher);
    // A template that changes on disk changes its output: its content is hashed too.
    for template in [
        css_template,
        less_template,
        scss_template,
        css_vars_template,
    ] {
        template.hash(&mut hasher);
        template
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .hash(&mut hasher);
    }
    (no_css, html, less, scss, cocos2d, json, json_format).hash(&mut hasher);
    (caat, css_vars, tailwind, dts).hash(&mut hasher);
    hasher.finish()
}

/// Data stored in the metadata file by the previous run.
#[derive(Debug, Default)]
pub(crate) struct Metadata {
    pub image_hash: Option<u64>,
    pub outputs_hash: Option<u64>,
    /// File name of the sprite image written by the previous run (see
    /// [`super::names::sprite_filename`]).
    pub image_file: Option<String>,
}

pub(super) fn read_metadata(config: &Config, sprite: &Sprite) -> Metadata {
    read_metadata_file(&get_meta_path(config, sprite))
}

/// Read a metadata file, one value per line: image hash, outputs hash, image file name.
pub(crate) fn read_metadata_file(path: &std::path::Path) -> Metadata {
    let Ok(file) = File::open(path) else {
        return Metadata::default();
    };
    let mut contents = String::new();
    if BufReader::new(file).read_to_string(&mut contents).is_err() {
        return Metadata::default();
    }
    let mut lines = contents.lines().map(str::trim);
    Metadata {
        image_hash: lines.next().and_then(|line| line.parse().ok()),
        outputs_hash: lines.next().and_then(|line| line.parse().ok()),
        image_file: lines
            .next()
            .filter(|line| !line.is_empty())
            .map(String::from),
    }
}

/// What has to be (re)generated for a sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Staleness {
    pub image: bool,
    pub outputs: bool,
}
impl Staleness {
    pub fn any(self) -> bool {
        self.image || self.outputs
    }
}

/// Compare the current inputs with the cached metadata to find what needs regeneration.
pub(crate) fn staleness(config: &Config, sprite: &Sprite) -> Staleness {
    staleness_from(config, sprite, &read_metadata(config, sprite))
}

pub(super) fn staleness_from(config: &Config, sprite: &Sprite, stored: &Metadata) -> Staleness {
    if config.force {
        log::debug!("Force flag set, regenerating sprite");
        return Staleness {
            image: !config.no_img,
            outputs: true,
        };
    }

    let name = sprite.name();

    let image = !config.no_img && {
        let missing_image = sprite_filenames(config, sprite)
            .into_iter()
            .find(|file| !config.image_dir().join(file).exists());
        if let Some(file) = missing_image {
            log::debug!("Sprite file {file} doesn't exist, generating");
            true
        } else if stored.image_hash != Some(compute_sprite_hash(config, sprite)) {
            log::debug!("Image hash mismatch, regenerating sprite image");
            true
        } else {
            false
        }
    };

    let missing_output = text_output_names(config, &name)
        .into_iter()
        .find(|file| !config.css_dir().join(file).exists());
    let outputs = if let Some(file) = missing_output {
        log::debug!("Output file {file} doesn't exist, generating");
        true
    } else if stored.outputs_hash != Some(compute_outputs_hash(config, sprite)) {
        log::debug!("Outputs hash mismatch, regenerating text outputs");
        true
    } else {
        false
    };

    Staleness { image, outputs }
}

/// Save the sprite hashes and image file name to the metadata file.
///
/// `image_hash` and `image_file` describe the image on disk, `None` when there is none
/// (e.g. `--no-img` on a fresh output directory), and `outputs_hash` is `None` when a text
/// output could not be written: the next run regenerates them.
pub(super) fn save_metadata(
    config: &Config,
    sprite: &Sprite,
    image_hash: Option<u64>,
    image_file: Option<&str>,
    outputs_hash: Option<u64>,
) -> std::io::Result<()> {
    let meta_path = get_meta_path(config, sprite);
    let image_line = image_hash.map(|hash| hash.to_string()).unwrap_or_default();
    let outputs_line = outputs_hash
        .map(|hash| hash.to_string())
        .unwrap_or_default();
    let image_file = image_file.unwrap_or_default();

    replace_file(&meta_path, |writer| {
        writer.write_all(format!("{image_line}\n{outputs_line}\n{image_file}\n").as_bytes())
    })
    .inspect_err(|e| log::error!("Cannot write {:?}: {}", meta_path, e))
}

#[cfg(test)]
mod tests {
    use super::super::names::{cache_buster, get_sprite_url, sprite_filename};
    use super::*;
    use crate::SpriteFormat;

    fn hashed_sprite() -> Sprite {
        Sprite {
            path: PathBuf::from("icons"),
            ratios: vec![1.0],
            images: vec![crate::ImageAndPosition {
                path: String::from("a.png"),
                computed_width: 4,
                computed_height: 4,
                source_hash: 42,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn test_layout_hash_is_cached_once() {
        let sprite = hashed_sprite();
        assert!(sprite.layout_hash.get().is_none());
        let first = compute_sprite_hash(&Config::default(), &sprite);
        assert_eq!(sprite.layout_hash.get(), Some(&layout_hash(&sprite)));
        assert_eq!(compute_sprite_hash(&Config::default(), &sprite), first);
    }

    #[test]
    fn test_cached_layout_hash_still_follows_the_config() {
        // Only the layout part is cached: image options still change the hash.
        let sprite = hashed_sprite();
        let base = compute_sprite_hash(&Config::default(), &sprite);
        for config in [
            Config {
                png8: true,
                ..Default::default()
            },
            Config {
                sprite_format: SpriteFormat::WebP,
                ..Default::default()
            },
            Config {
                padding: Some(String::from("2")),
                ..Default::default()
            },
        ] {
            assert_ne!(compute_sprite_hash(&config, &sprite), base, "{config:?}");
        }
    }

    #[test]
    fn test_cache_buster_is_consistent_across_outputs() {
        let sprite = hashed_sprite();
        let config = Config {
            cachebuster_filename: true,
            ..Default::default()
        };
        let buster = cache_buster(&config, &sprite);
        assert_eq!(
            sprite_filename(&config, &sprite),
            format!("icons_{buster}.png")
        );
        assert!(get_sprite_url(&config, &sprite).contains(&buster));
        assert_eq!(
            format!(
                "{:08x}",
                compute_sprite_hash(&config, &sprite) & 0xffff_ffff
            ),
            buster
        );
    }
}
