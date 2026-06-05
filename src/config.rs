use serde::Deserialize;
use std::path::Path;

use crate::{Algorithm, Config, JsonFormat, Ordering, SpriteFormat};

/// TOML-deserializable config with all fields optional.
/// Loaded from `spriters.toml` in the source directory.
#[derive(Debug, Default, Deserialize)]
pub struct FileConfig {
    pub algorithm: Option<String>,
    pub ordering: Option<String>,
    pub crop: Option<bool>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub padding: Option<String>,
    pub margin: Option<String>,
    pub ratios: Option<String>,
    pub cachebuster: Option<bool>,
    pub cachebuster_filename: Option<bool>,
    pub cachebuster_filename_only_sprites: Option<bool>,
    pub force: Option<bool>,
    pub follow_links: Option<bool>,
    pub namespace: Option<String>,
    pub sprite_namespace: Option<String>,
    pub separator: Option<String>,
    pub pseudo_class_separator: Option<String>,
    pub url: Option<String>,
    pub json: Option<bool>,
    pub json_format: Option<String>,
    pub sprite_format: Option<String>,
    pub caat: Option<bool>,
    pub html: Option<bool>,
    pub cocos2d: Option<bool>,
    pub less: Option<bool>,
    pub scss: Option<bool>,
    pub no_css: Option<bool>,
    pub no_img: Option<bool>,
    pub png8: Option<bool>,
    pub no_duplicate: Option<bool>,
    pub recursive: Option<bool>,
    pub project: Option<bool>,
    pub exclude: Option<Vec<String>>,
    pub css_vars: Option<bool>,
}

/// Load a `spriters.toml` from `dir` if it exists.
pub fn load_file_config(dir: &Path) -> Option<FileConfig> {
    let path = dir.join("spriters.toml");
    if !path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(&path)
        .map_err(|e| log::warn!("Could not read spriters.toml: {}", e))
        .ok()?;
    toml::from_str(&contents)
        .map_err(|e| log::warn!("Could not parse spriters.toml: {}", e))
        .ok()
}

/// Apply env vars (SPRITERS_*) to config. Lower priority than CLI and file config.
pub fn apply_env_vars(config: &mut Config) {
    use std::env;

    if let Ok(v) = env::var("SPRITERS_ALGORITHM") {
        if let Ok(a) = parse_algorithm(&v) {
            config.algorithm = a;
        }
    }
    if let Ok(v) = env::var("SPRITERS_ORDERING") {
        if let Ok(o) = v.parse::<Ordering>() {
            config.ordering = o;
        }
    }
    if let Ok(v) = env::var("SPRITERS_CROP") {
        if let Ok(b) = parse_bool(&v) {
            config.crop = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_MAX_WIDTH") {
        if let Ok(n) = v.parse::<u32>() {
            config.max_width = Some(n);
        }
    }
    if let Ok(v) = env::var("SPRITERS_MAX_HEIGHT") {
        if let Ok(n) = v.parse::<u32>() {
            config.max_height = Some(n);
        }
    }
    if let Ok(v) = env::var("SPRITERS_PADDING") {
        config.padding = Some(v);
    }
    if let Ok(v) = env::var("SPRITERS_MARGIN") {
        config.margin = Some(v);
    }
    if let Ok(v) = env::var("SPRITERS_RATIOS") {
        config.ratios = Some(v);
    }
    if let Ok(v) = env::var("SPRITERS_NAMESPACE") {
        config.namespace = Some(v);
    }
    if let Ok(v) = env::var("SPRITERS_SPRITE_NAMESPACE") {
        config.sprite_namespace = Some(v);
    }
    if let Ok(v) = env::var("SPRITERS_SEPARATOR") {
        config.separator = v;
    }
    if let Ok(v) = env::var("SPRITERS_URL") {
        config.url = Some(v);
    }
    if let Ok(v) = env::var("SPRITERS_JSON_FORMAT") {
        config.json_format = if v.to_lowercase() == "hash" {
            JsonFormat::Hash
        } else {
            JsonFormat::Array
        };
    }
    if let Ok(v) = env::var("SPRITERS_CACHEBUSTER") {
        if let Ok(b) = parse_bool(&v) {
            config.cachebuster = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_CACHEBUSTER_FILENAME") {
        if let Ok(b) = parse_bool(&v) {
            config.cachebuster_filename = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_CACHEBUSTER_FILENAME_ONLY_SPRITES") {
        if let Ok(b) = parse_bool(&v) {
            config.cachebuster_filename_only_sprites = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_FORCE") {
        if let Ok(b) = parse_bool(&v) {
            config.force = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_FOLLOW_LINKS") {
        if let Ok(b) = parse_bool(&v) {
            config.follow_links = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_RECURSIVE") {
        if let Ok(b) = parse_bool(&v) {
            config.recursive = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_PROJECT") {
        if let Ok(b) = parse_bool(&v) {
            config.project = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_NO_CSS") {
        if let Ok(b) = parse_bool(&v) {
            config.no_css = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_NO_IMG") {
        if let Ok(b) = parse_bool(&v) {
            config.no_img = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_PNG8") {
        if let Ok(b) = parse_bool(&v) {
            config.png8 = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_HTML") {
        if let Ok(b) = parse_bool(&v) {
            config.html = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_JSON") {
        if let Ok(b) = parse_bool(&v) {
            config.json = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_CAAT") {
        if let Ok(b) = parse_bool(&v) {
            config.caat = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_LESS") {
        if let Ok(b) = parse_bool(&v) {
            config.less = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_SCSS") {
        if let Ok(b) = parse_bool(&v) {
            config.scss = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_COCOS2D") {
        if let Ok(b) = parse_bool(&v) {
            config.cocos2d = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_NO_DUPLICATE") {
        if let Ok(b) = parse_bool(&v) {
            config.no_duplicate = b;
        }
    }
    if let Ok(v) = env::var("SPRITERS_PSEUDO_CLASS_SEPARATOR") {
        config.pseudo_class_separator = Some(v);
    }
    if let Ok(v) = env::var("SPRITERS_FORMAT") {
        config.sprite_format = parse_sprite_format(&v);
    }
    if let Ok(v) = env::var("SPRITERS_EXCLUDE") {
        config.exclude = v.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
    }
    if let Ok(v) = env::var("SPRITERS_CSS_VARS") {
        if let Ok(b) = parse_bool(&v) {
            config.css_vars = b;
        }
    }
}

/// Overlay file config values onto config. Higher priority than env vars and CLI.
pub fn apply_file_config(config: &mut Config, fc: FileConfig) {
    if let Some(v) = fc.algorithm {
        if let Ok(a) = parse_algorithm(&v) {
            config.algorithm = a;
        }
    }
    if let Some(v) = fc.ordering {
        if let Ok(o) = v.parse::<Ordering>() {
            config.ordering = o;
        }
    }
    if let Some(v) = fc.crop {
        config.crop = v;
    }
    if let Some(v) = fc.max_width {
        config.max_width = Some(v);
    }
    if let Some(v) = fc.max_height {
        config.max_height = Some(v);
    }
    if let Some(v) = fc.padding {
        config.padding = Some(v);
    }
    if let Some(v) = fc.margin {
        config.margin = Some(v);
    }
    if let Some(v) = fc.ratios {
        config.ratios = Some(v);
    }
    if let Some(v) = fc.cachebuster {
        config.cachebuster = v;
    }
    if let Some(v) = fc.cachebuster_filename {
        config.cachebuster_filename = v;
    }
    if let Some(v) = fc.cachebuster_filename_only_sprites {
        config.cachebuster_filename_only_sprites = v;
    }
    if let Some(v) = fc.force {
        config.force = v;
    }
    if let Some(v) = fc.follow_links {
        config.follow_links = v;
    }
    if let Some(v) = fc.namespace {
        config.namespace = Some(v);
    }
    if let Some(v) = fc.sprite_namespace {
        config.sprite_namespace = Some(v);
    }
    if let Some(v) = fc.separator {
        config.separator = v;
    }
    if let Some(v) = fc.pseudo_class_separator {
        config.pseudo_class_separator = Some(v);
    }
    if let Some(v) = fc.url {
        config.url = Some(v);
    }
    if let Some(v) = fc.json {
        config.json = v;
    }
    if let Some(v) = fc.json_format {
        config.json_format = if v.to_lowercase() == "hash" {
            JsonFormat::Hash
        } else {
            JsonFormat::Array
        };
    }
    if let Some(v) = fc.sprite_format {
        config.sprite_format = parse_sprite_format(&v);
    }
    if let Some(v) = fc.caat {
        config.caat = v;
    }
    if let Some(v) = fc.html {
        config.html = v;
    }
    if let Some(v) = fc.cocos2d {
        config.cocos2d = v;
    }
    if let Some(v) = fc.less {
        config.less = v;
    }
    if let Some(v) = fc.scss {
        config.scss = v;
    }
    if let Some(v) = fc.no_css {
        config.no_css = v;
    }
    if let Some(v) = fc.no_img {
        config.no_img = v;
    }
    if let Some(v) = fc.png8 {
        config.png8 = v;
    }
    if let Some(v) = fc.no_duplicate {
        config.no_duplicate = v;
    }
    if let Some(v) = fc.recursive {
        config.recursive = v;
    }
    if let Some(v) = fc.project {
        config.project = v;
    }
    if let Some(v) = fc.exclude {
        config.exclude.extend(v);
    }
    if let Some(v) = fc.css_vars {
        config.css_vars = v;
    }
}

fn parse_algorithm(s: &str) -> Result<Algorithm, ()> {
    match s.to_lowercase().as_str() {
        "square" => Ok(Algorithm::Square),
        "vertical" => Ok(Algorithm::Vertical),
        "horizontal" => Ok(Algorithm::Horizontal),
        "diagonal" => Ok(Algorithm::Diagonal),
        "verticalright" | "vertical-right" => Ok(Algorithm::VerticalRight),
        "horizontalbottom" | "horizontal-bottom" => Ok(Algorithm::HorizontalBottom),
        _ => Err(()),
    }
}

fn parse_bool(s: &str) -> Result<bool, ()> {
    match s.to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(()),
    }
}

pub(crate) fn parse_sprite_format(s: &str) -> SpriteFormat {
    match s.to_lowercase().as_str() {
        "webp" => SpriteFormat::WebP,
        "avif" => SpriteFormat::Avif,
        _ => SpriteFormat::Png,
    }
}
