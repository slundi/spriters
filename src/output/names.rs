//! File names and URLs of the sprite images, and recognition of the generated files.

use std::path::PathBuf;

use super::files::temp_target;
use super::meta::compute_sprite_hash;
use crate::{Config, Sprite};

/// Deterministic cache buster: lower 32 bits of the sprite hash, hex-encoded.
pub(super) fn cache_buster(config: &Config, sprite: &Sprite) -> String {
    format!("{:08x}", compute_sprite_hash(config, sprite) & 0xffff_ffff)
}

/// Whether the cache buster is part of the sprite image file name.
fn hashed_sprite_filename(config: &Config) -> bool {
    config.cachebuster_filename || config.cachebuster_filename_only_sprites
}

/// Suffix of the image file for `ratio`: none for 1×, otherwise `@2x`, `@1.5x`…
fn ratio_suffix(ratio: f32) -> String {
    if ratio == 1.0 {
        String::new()
    } else {
        format!("@{ratio}x")
    }
}

/// Smallest ratio: its image is the one referenced by default.
fn base_ratio(sprite: &Sprite) -> f32 {
    sprite.ratios.first().copied().unwrap_or(1.0)
}

/// Whether the sprite has other images than a single 1× one (`--ratios` / `--retina`).
pub(super) fn has_ratios(sprite: &Sprite) -> bool {
    sprite.ratios.len() > 1 || base_ratio(sprite) != 1.0
}

/// File name of the sprite image at `ratio`: `name[_<buster>][@<ratio>x].ext`, the buster
/// being added with `--cachebuster-filename` / `--cachebuster-filename-only-sprites`.
///
/// Single source of truth for the written files and every reference to them.
pub(super) fn sprite_filename_at(config: &Config, sprite: &Sprite, ratio: f32) -> String {
    let name = sprite.name();
    let ext = config.sprite_format.extension();
    let suffix = ratio_suffix(ratio);
    if hashed_sprite_filename(config) {
        format!("{name}_{}{suffix}.{ext}", cache_buster(config, sprite))
    } else {
        format!("{name}{suffix}.{ext}")
    }
}

/// File name of the sprite image referenced by default (smallest ratio).
pub(super) fn sprite_filename(config: &Config, sprite: &Sprite) -> String {
    sprite_filename_at(config, sprite, base_ratio(sprite))
}

/// File names of every sprite image, one per ratio, smallest ratio first.
pub(super) fn sprite_filenames(config: &Config, sprite: &Sprite) -> Vec<String> {
    sprite
        .ratios
        .iter()
        .map(|&ratio| sprite_filename_at(config, sprite, ratio))
        .collect()
}

/// Get the sprite image URL for use in CSS.
/// - If `config.url` is set, uses that as base URL
/// - Cache buster is derived deterministically from the sprite content hash.
pub(super) fn get_sprite_url(config: &Config, sprite: &Sprite) -> String {
    sprite_file_url(config, sprite, sprite_filename(config, sprite))
}

/// URL of the sprite image file `filename`, see [`get_sprite_url`].
pub(super) fn sprite_file_url(config: &Config, sprite: &Sprite, mut filename: String) -> String {
    if config.cachebuster && !hashed_sprite_filename(config) {
        filename = format!("{filename}?{}", cache_buster(config, sprite));
    }

    // If custom URL is provided, join it with the filename
    match &config.url {
        Some(base_url) => {
            if base_url.ends_with('/') {
                format!("{}{}", base_url, filename)
            } else {
                format!("{}/{}", base_url, filename)
            }
        }
        None => format!("{}{filename}", image_dir_url(config)),
    }
}

/// `path` made absolute, symbolic links resolved when it exists.
fn resolved(path: &std::path::Path) -> PathBuf {
    path.canonicalize()
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// URL of the image directory relative to the text outputs (`--img` and `--css`): empty
/// when they are the same directory, otherwise a path ending with `/` (`../img/`).
fn image_dir_url(config: &Config) -> String {
    let (from, to) = (resolved(config.css_dir()), resolved(config.image_dir()));
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        // Nothing in common (another drive): only an absolute path reaches it.
        return format!("{}/", config.image_dir().display());
    }
    let parts: Vec<String> = std::iter::repeat_n(String::from(".."), from.len() - common)
        .chain(
            to[common..]
                .iter()
                .map(|component| component.as_os_str().to_string_lossy().into_owned()),
        )
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{}/", parts.join("/"))
    }
}

/// A sprite image file name `name[_<8 hex>][@<ratio>x].<ext>` (see [`sprite_filename_at`]).
pub(super) struct SpriteImageName<'a> {
    pub(super) hashed: bool,
    pub(super) ratio_suffix: bool,
    pub(super) ext: &'a str,
}

/// Parse `file_name` as an image of the sprite `name`, `None` if it is not one.
pub(super) fn parse_sprite_image_name<'a>(
    file_name: &'a str,
    name: &str,
) -> Option<SpriteImageName<'a>> {
    let (stem, ext) = file_name.strip_prefix(name)?.rsplit_once('.')?;
    if !SPRITE_EXTENSIONS.contains(&ext) {
        return None;
    }
    let (stem, ratio_suffix) = match stem.split_once('@') {
        Some((stem, ratio)) => {
            ratio
                .strip_suffix('x')?
                .parse::<f32>()
                .ok()
                .filter(|ratio| ratio.is_finite() && *ratio > 0.0)?;
            (stem, true)
        }
        None => (stem, false),
    };
    let hashed = match stem.strip_prefix('_') {
        Some(buster) if buster.len() == 8 && buster.bytes().all(|b| b.is_ascii_hexdigit()) => true,
        _ if stem.is_empty() => false,
        _ => return None,
    };
    Some(SpriteImageName {
        hashed,
        ratio_suffix,
        ext,
    })
}

/// Name of the metadata file for the sprite `name`.
pub(crate) fn meta_file_name(name: &str) -> String {
    format!(".{name}.spriters")
}

/// Sprite image extensions, see [`crate::SpriteFormat::extension`].
const SPRITE_EXTENSIONS: [&str; 3] = ["png", "webp", "avif"];

/// Every suffix (after `<sprite name>.`) of a file spriters may write, whatever the options:
/// the sprite image and all text outputs (see [`super::text_output_names`]).
const GENERATED_SUFFIXES: [&str; 13] = [
    "png",
    "webp",
    "avif",
    "css",
    "less",
    "scss",
    "json",
    "html",
    "plist",
    "caat.json",
    "vars.css",
    "tailwind.json",
    "d.ts",
];

/// Whether `file_name` is a file spriters writes for one of `sprite_names`: sprite image
/// (plain or cache-busted, see [`sprite_filename`]), text output, metadata file, or the
/// temporary file of one of them (see [`super::files::temp_path`]).
pub(crate) fn is_generated_file_name(file_name: &str, sprite_names: &[String]) -> bool {
    if let Some(target) = temp_target(file_name) {
        return is_generated_file_name(target, sprite_names);
    }
    sprite_names.iter().any(|name| {
        file_name == meta_file_name(name)
            || file_name
                .strip_prefix(name.as_str())
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|suffix| GENERATED_SUFFIXES.contains(&suffix))
            || parse_sprite_image_name(file_name, name).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SpriteFormat;

    #[test]
    fn test_temp_files_are_generated_files() {
        // Otherwise `--watch` regenerates on its own temporary files, and discovery could
        // see them when the output directory is the source.
        let names = vec![String::from("icons")];
        for file in [
            ".icons.css.42.tmp",
            ".icons.png.42.tmp",
            ".icons_0123abcd@2x.png.42.tmp",
            "..icons.spriters.42.tmp",
        ] {
            assert!(is_generated_file_name(file, &names), "{file}");
        }
        for file in [".other.css.42.tmp", ".red.png.42.tmp", ".icons.css.tmp"] {
            assert!(!is_generated_file_name(file, &names), "{file}");
        }
    }

    #[test]
    fn test_cache_buster_deterministic() {
        use crate::{Config, ImageAndPosition, Sprite};
        use std::path::PathBuf;

        let make_sprite = || {
            let config = Config::default();

            Sprite {
                config: config.clone(),
                path: PathBuf::from("america"),
                ratios: vec![1.0],
                images: vec![ImageAndPosition {
                    path: String::from("us.png"),
                    image: image::DynamicImage::new_rgba8(100, 50),
                    computed_width: 100,
                    computed_height: 50,
                    x: 0,
                    y: 0,
                    aliases: vec![],
                    source_hash: 0,
                }],
                margin: crate::util::Spacing::default(),
                padding: crate::util::Spacing::default(),
                failed_images: Vec::new(),
                layout_hash: Default::default(),
            }
        };

        // No cachebuster: URL is just the filename
        let config = Config::default();
        let sprite = make_sprite();
        let url = get_sprite_url(&config, &sprite);
        assert_eq!(url, "america.png");

        // cachebuster query param: deterministic (same hash every call)
        let config_q = Config {
            cachebuster: true,
            ..Default::default()
        };
        let url1 = get_sprite_url(&config_q, &make_sprite());
        let url2 = get_sprite_url(&config_q, &make_sprite());
        assert_eq!(url1, url2, "cache buster must be deterministic");
        assert!(url1.contains('?'), "query-param buster must contain '?'");

        // cachebuster filename: deterministic
        let config_f = Config {
            cachebuster_filename: true,
            ..Default::default()
        };
        let url3 = get_sprite_url(&config_f, &make_sprite());
        let url4 = get_sprite_url(&config_f, &make_sprite());
        assert_eq!(url3, url4, "filename buster must be deterministic");
        assert!(url3.contains('_'), "filename buster must contain '_'");
        assert!(url3.ends_with(".png"), "filename buster must end with .png");
    }

    #[test]
    fn test_every_written_file_is_recognised_as_generated() {
        // Guards against a new output whose name the self-inclusion filter would miss.
        let names = vec![String::from("america")];
        let sprite = Sprite {
            path: PathBuf::from("america"),
            ratios: vec![1.0],
            ..Default::default()
        };
        let every_output = Config {
            less: true,
            scss: true,
            json: true,
            html: true,
            cocos2d: true,
            caat: true,
            css_vars: true,
            tailwind: true,
            dts: true,
            ..Default::default()
        };
        for file in super::super::text_output_names(&every_output, "america") {
            assert!(is_generated_file_name(&file, &names), "{file}");
        }
        assert!(is_generated_file_name(&meta_file_name("america"), &names));
        let retina = Sprite {
            path: PathBuf::from("america"),
            ratios: vec![1.0, 1.5, 2.0],
            ..Default::default()
        };
        for format in [SpriteFormat::Png, SpriteFormat::WebP, SpriteFormat::Avif] {
            for hashed in [false, true] {
                let config = Config {
                    sprite_format: format,
                    cachebuster_filename: hashed,
                    ..Default::default()
                };
                let file = sprite_filename(&config, &sprite);
                assert!(is_generated_file_name(&file, &names), "{file}");
                for file in sprite_filenames(&config, &retina) {
                    assert!(is_generated_file_name(&file, &names), "{file}");
                }
            }
        }
    }

    #[test]
    fn test_sprite_filenames_per_ratio() {
        let sprite = Sprite {
            path: PathBuf::from("icons"),
            ratios: vec![1.0, 1.5, 2.0],
            ..Default::default()
        };
        assert_eq!(
            sprite_filenames(&Config::default(), &sprite),
            vec!["icons.png", "icons@1.5x.png", "icons@2x.png"]
        );
        let hashed = Config {
            cachebuster_filename: true,
            ..Default::default()
        };
        let files = sprite_filenames(&hashed, &sprite);
        assert!(files[0].starts_with("icons_") && files[0].ends_with(".png"));
        assert!(files[2].starts_with("icons_") && files[2].ends_with("@2x.png"));

        // Without 1×, the smallest ratio is the image referenced by default.
        let only_2x = Sprite {
            path: PathBuf::from("icons"),
            ratios: vec![2.0],
            ..Default::default()
        };
        assert_eq!(
            sprite_filename(&Config::default(), &only_2x),
            "icons@2x.png"
        );
    }

    #[test]
    fn test_source_images_are_not_generated_files() {
        let names = vec![String::from("icons")];
        for file in [
            "arrow.png",
            "icons2.png",
            "iconsx.css",
            "icons.hover.png",
            "icons_1234.png",
            "icons_12345678.gif",
            "icons_zzzzzzzz.png",
            "icons@x.png",
            "icons@0x.png",
            "icons@2.png",
            "icons@2x_12345678.png",
            ".other.spriters",
        ] {
            assert!(!is_generated_file_name(file, &names), "{file}");
        }
        assert!(!is_generated_file_name("icons.png", &[]));
    }

    #[test]
    fn test_image_dir_url_is_relative_to_the_css() {
        let root =
            std::env::temp_dir().join(format!("spriters_image_dir_url_{}", std::process::id()));
        for dir in ["css/sub", "img", "css/img"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        let url = |css: Option<&str>, img: Option<&str>| {
            image_dir_url(&Config {
                output: root.clone(),
                css: css.map(|dir| root.join(dir)),
                img: img.map(|dir| root.join(dir)),
                ..Default::default()
            })
        };
        assert_eq!(url(None, None), "");
        assert_eq!(url(Some("css"), Some("css")), "");
        assert_eq!(url(Some("css"), Some("img")), "../img/");
        assert_eq!(url(Some("css/sub"), Some("img")), "../../img/");
        assert_eq!(url(Some("css"), Some("css/img")), "img/");
        assert_eq!(url(Some("css/img"), None), "../../");
        assert_eq!(url(None, Some("css/sub")), "css/sub/");
        // Spelled differently, same directory.
        assert_eq!(url(Some("css/sub/.."), Some("css")), "");
        std::fs::remove_dir_all(&root).ok();
    }
}
