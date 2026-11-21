//! Where the configuration comes from besides the command line: `spriters.toml` and the
//! `SPRITERS_*` environment variables.

use serde::Deserialize;
use std::path::Path;

use crate::{Algorithm, Config, JsonFormat, Ordering, SpriteFormat};
use image::imageops::FilterType;

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
    pub dry_run: Option<bool>,
    pub check: Option<bool>,
    pub tailwind: Option<bool>,
    pub dts: Option<bool>,
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

/// Build the effective configuration from every source, lowest priority first:
/// defaults < `spriters.toml` < `SPRITERS_*` env vars (via `env`) < command line (`cli`).
///
/// `cli` must only set the options actually given on the command line. It is applied a
/// first time on its own to find the source directory, where `spriters.toml` is read.
/// `exclude` patterns accumulate across all sources instead of replacing each other.
pub fn resolve(cli: impl Fn(&mut Config), env: impl Fn(&str) -> Option<String>) -> Config {
    let mut cli_only = Config::default();
    cli(&mut cli_only);

    let mut config = Config::default();
    if let Some(file_config) = load_file_config(&cli_only.source) {
        apply_file_config(&mut config, file_config);
    }
    apply_env_with(&mut config, env);
    cli(&mut config);
    config
}

/// Apply env vars (SPRITERS_*) to config. Higher priority than `spriters.toml`, lower than
/// the command line (see [`resolve`]).
pub fn apply_env_vars(config: &mut Config) {
    apply_env_with(config, |key| std::env::var(key).ok());
}

/// Apply SPRITERS_* variables obtained through `lookup` to config.
///
/// [`apply_env_vars`] reads them from the process environment; tests pass their own
/// lookup instead, since mutating the environment is `unsafe` and racy across threads.
pub fn apply_env_with(config: &mut Config, lookup: impl Fn(&str) -> Option<String>) {
    let algorithm = lookup("SPRITERS_ALGORITHM");
    if let Some(value) = checked(ENV, "SPRITERS_ALGORITHM", algorithm, parse_algorithm) {
        config.algorithm = value;
    }
    let ordering = lookup("SPRITERS_ORDERING");
    if let Some(value) = checked(ENV, "SPRITERS_ORDERING", ordering, parse_ordering) {
        config.ordering = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_CROP", lookup("SPRITERS_CROP"), parse_bool) {
        config.crop = value;
    }
    let max_width = lookup("SPRITERS_MAX_WIDTH");
    if let Some(value) = checked(ENV, "SPRITERS_MAX_WIDTH", max_width, parse_number) {
        config.max_width = Some(value);
    }
    let max_height = lookup("SPRITERS_MAX_HEIGHT");
    if let Some(value) = checked(ENV, "SPRITERS_MAX_HEIGHT", max_height, parse_number) {
        config.max_height = Some(value);
    }
    if let Some(v) = lookup("SPRITERS_PADDING") {
        config.padding = Some(v);
    }
    if let Some(v) = lookup("SPRITERS_MARGIN") {
        config.margin = Some(v);
    }
    if let Some(v) = lookup("SPRITERS_RATIOS") {
        config.ratios = Some(v);
    }
    if let Some(v) = lookup("SPRITERS_NAMESPACE") {
        config.namespace = Some(v);
    }
    if let Some(v) = lookup("SPRITERS_SPRITE_NAMESPACE") {
        config.sprite_namespace = Some(v);
    }
    if let Some(v) = lookup("SPRITERS_SEPARATOR") {
        config.separator = v;
    }
    if let Some(v) = lookup("SPRITERS_URL") {
        config.url = Some(v);
    }
    let json_format = lookup("SPRITERS_JSON_FORMAT");
    if let Some(value) = checked(ENV, "SPRITERS_JSON_FORMAT", json_format, parse_json_format) {
        config.json_format = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_CACHEBUSTER",
        lookup("SPRITERS_CACHEBUSTER"),
        parse_bool,
    ) {
        config.cachebuster = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_CACHEBUSTER_FILENAME",
        lookup("SPRITERS_CACHEBUSTER_FILENAME"),
        parse_bool,
    ) {
        config.cachebuster_filename = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_CACHEBUSTER_FILENAME_ONLY_SPRITES",
        lookup("SPRITERS_CACHEBUSTER_FILENAME_ONLY_SPRITES"),
        parse_bool,
    ) {
        config.cachebuster_filename_only_sprites = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_FORCE", lookup("SPRITERS_FORCE"), parse_bool) {
        config.force = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_FOLLOW_LINKS",
        lookup("SPRITERS_FOLLOW_LINKS"),
        parse_bool,
    ) {
        config.follow_links = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_RECURSIVE",
        lookup("SPRITERS_RECURSIVE"),
        parse_bool,
    ) {
        config.recursive = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_PROJECT",
        lookup("SPRITERS_PROJECT"),
        parse_bool,
    ) {
        config.project = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_NO_CSS",
        lookup("SPRITERS_NO_CSS"),
        parse_bool,
    ) {
        config.no_css = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_NO_IMG",
        lookup("SPRITERS_NO_IMG"),
        parse_bool,
    ) {
        config.no_img = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_PNG8", lookup("SPRITERS_PNG8"), parse_bool) {
        config.png8 = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_HTML", lookup("SPRITERS_HTML"), parse_bool) {
        config.html = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_JSON", lookup("SPRITERS_JSON"), parse_bool) {
        config.json = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_CAAT", lookup("SPRITERS_CAAT"), parse_bool) {
        config.caat = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_LESS", lookup("SPRITERS_LESS"), parse_bool) {
        config.less = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_SCSS", lookup("SPRITERS_SCSS"), parse_bool) {
        config.scss = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_COCOS2D",
        lookup("SPRITERS_COCOS2D"),
        parse_bool,
    ) {
        config.cocos2d = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_NO_DUPLICATE",
        lookup("SPRITERS_NO_DUPLICATE"),
        parse_bool,
    ) {
        config.no_duplicate = value;
    }
    if let Some(v) = lookup("SPRITERS_PSEUDO_CLASS_SEPARATOR") {
        config.pseudo_class_separator = Some(v);
    }
    let format = lookup("SPRITERS_FORMAT");
    if let Some(value) = checked(ENV, "SPRITERS_FORMAT", format, parse_sprite_format) {
        config.sprite_format = value;
    }
    if let Some(v) = lookup("SPRITERS_EXCLUDE") {
        config.exclude.extend(
            v.split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
        );
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_CSS_VARS",
        lookup("SPRITERS_CSS_VARS"),
        parse_bool,
    ) {
        config.css_vars = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_DRY_RUN",
        lookup("SPRITERS_DRY_RUN"),
        parse_bool,
    ) {
        config.dry_run = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_CHECK", lookup("SPRITERS_CHECK"), parse_bool) {
        config.check = value;
    }
    if let Some(value) = checked(
        ENV,
        "SPRITERS_TAILWIND",
        lookup("SPRITERS_TAILWIND"),
        parse_bool,
    ) {
        config.tailwind = value;
    }
    if let Some(value) = checked(ENV, "SPRITERS_DTS", lookup("SPRITERS_DTS"), parse_bool) {
        config.dts = value;
    }
}

/// Overlay file config values onto config. Lowest priority after the defaults: env vars
/// and the command line override it (see [`resolve`]).
pub fn apply_file_config(config: &mut Config, fc: FileConfig) {
    if let Some(value) = checked(FILE, "algorithm", fc.algorithm, parse_algorithm) {
        config.algorithm = value;
    }
    if let Some(value) = checked(FILE, "ordering", fc.ordering, parse_ordering) {
        config.ordering = value;
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
    if let Some(value) = checked(FILE, "json_format", fc.json_format, parse_json_format) {
        config.json_format = value;
    }
    if let Some(value) = checked(FILE, "sprite_format", fc.sprite_format, parse_sprite_format) {
        config.sprite_format = value;
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
    if let Some(v) = fc.dry_run {
        config.dry_run = v;
    }
    if let Some(v) = fc.check {
        config.check = v;
    }
    if let Some(v) = fc.tailwind {
        config.tailwind = v;
    }
    if let Some(v) = fc.dts {
        config.dts = v;
    }
}

/// A setting whose value is not one of the accepted ones.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {setting} '{value}', expected {expected}")]
pub struct InvalidValue {
    /// What the value is for, e.g. `image format`.
    pub setting: &'static str,
    pub value: String,
    /// The accepted values.
    pub expected: &'static str,
}

fn invalid(setting: &'static str, value: &str, expected: &'static str) -> InvalidValue {
    InvalidValue {
        setting,
        value: value.to_owned(),
        expected,
    }
}

/// Origins of a setting, for the messages of [`checked`].
const ENV: &str = "the environment";
const FILE: &str = "spriters.toml";

/// `value` of the setting `key` from `origin`, parsed with `parse`. An invalid value is
/// reported and ignored (the command line rejects it instead); it used to fall back to a
/// default in silence.
fn checked<T>(
    origin: &str,
    key: &str,
    value: Option<String>,
    parse: fn(&str) -> Result<T, InvalidValue>,
) -> Option<T> {
    parse(&value?)
        .inspect_err(|e| log::warn!("Ignoring {key} from {origin}: {e}"))
        .ok()
}

/// Parse an algorithm name (case-insensitive).
pub fn parse_algorithm(s: &str) -> Result<Algorithm, InvalidValue> {
    match s.to_lowercase().as_str() {
        "square" => Ok(Algorithm::Square),
        "vertical" => Ok(Algorithm::Vertical),
        "horizontal" => Ok(Algorithm::Horizontal),
        "diagonal" => Ok(Algorithm::Diagonal),
        "verticalright" | "vertical-right" => Ok(Algorithm::VerticalRight),
        "horizontalbottom" | "horizontal-bottom" => Ok(Algorithm::HorizontalBottom),
        _ => Err(invalid(
            "algorithm",
            s,
            "square, vertical, horizontal, diagonal, vertical-right or horizontal-bottom",
        )),
    }
}

/// Parse an ordering name (case-insensitive), see [`Ordering`].
pub fn parse_ordering(s: &str) -> Result<Ordering, InvalidValue> {
    s.parse::<Ordering>().map_err(|()| {
        invalid(
            "ordering",
            s,
            "maxside, width, height, area or filename, prefixed with '-' for reverse",
        )
    })
}

/// Parse a sprite image format (case-insensitive).
pub fn parse_sprite_format(s: &str) -> Result<SpriteFormat, InvalidValue> {
    match s.to_lowercase().as_str() {
        "png" => Ok(SpriteFormat::Png),
        "webp" => Ok(SpriteFormat::WebP),
        "avif" => Ok(SpriteFormat::Avif),
        _ => Err(invalid("image format", s, "png, webp or avif")),
    }
}

/// Parse a JSON output format (case-insensitive).
pub fn parse_json_format(s: &str) -> Result<JsonFormat, InvalidValue> {
    match s.to_lowercase().as_str() {
        "array" => Ok(JsonFormat::Array),
        "hash" => Ok(JsonFormat::Hash),
        _ => Err(invalid("JSON format", s, "array or hash")),
    }
}

/// Parse a resize filter name (case-insensitive).
pub fn parse_resize_algorithm(s: &str) -> Result<FilterType, InvalidValue> {
    match s.to_lowercase().as_str() {
        "nearest" => Ok(FilterType::Nearest),
        "triangle" => Ok(FilterType::Triangle),
        "gaussian" => Ok(FilterType::Gaussian),
        "lanczos3" => Ok(FilterType::Lanczos3),
        "catmullrom" | "catmull-rom" => Ok(FilterType::CatmullRom),
        _ => Err(invalid(
            "resize algorithm",
            s,
            "nearest, triangle, gaussian, lanczos3 or catmullrom",
        )),
    }
}

fn parse_bool(s: &str) -> Result<bool, InvalidValue> {
    match s.to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(invalid("boolean", s, "true/false, yes/no, on/off or 1/0")),
    }
}

fn parse_number(s: &str) -> Result<u32, InvalidValue> {
    s.parse()
        .map_err(|_| invalid("number", s, "a non-negative integer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Algorithm, Config, SpriteFormat};

    #[test]
    fn test_parse_algorithm_valid() {
        assert_eq!(parse_algorithm("square"), Ok(Algorithm::Square));
        assert_eq!(parse_algorithm("SQUARE"), Ok(Algorithm::Square));
        assert_eq!(parse_algorithm("vertical"), Ok(Algorithm::Vertical));
        assert_eq!(parse_algorithm("horizontal"), Ok(Algorithm::Horizontal));
        assert_eq!(parse_algorithm("diagonal"), Ok(Algorithm::Diagonal));
        assert_eq!(
            parse_algorithm("verticalright"),
            Ok(Algorithm::VerticalRight)
        );
        assert_eq!(
            parse_algorithm("vertical-right"),
            Ok(Algorithm::VerticalRight)
        );
        assert_eq!(
            parse_algorithm("horizontalbottom"),
            Ok(Algorithm::HorizontalBottom)
        );
        assert_eq!(
            parse_algorithm("horizontal-bottom"),
            Ok(Algorithm::HorizontalBottom)
        );
    }

    #[test]
    fn test_parse_algorithm_invalid() {
        assert!(parse_algorithm("unknown").is_err());
        assert!(parse_algorithm("").is_err());
        assert!(parse_algorithm("vert").is_err());
    }

    #[test]
    fn test_parse_sprite_format() {
        assert_eq!(parse_sprite_format("webp"), Ok(SpriteFormat::WebP));
        assert_eq!(parse_sprite_format("WEBP"), Ok(SpriteFormat::WebP));
        assert_eq!(parse_sprite_format("avif"), Ok(SpriteFormat::Avif));
        assert_eq!(parse_sprite_format("AVIF"), Ok(SpriteFormat::Avif));
        assert_eq!(parse_sprite_format("png"), Ok(SpriteFormat::Png));
        // Review 6.8: unknown values used to give PNG.
        assert!(parse_sprite_format("unknown").is_err());
        assert!(parse_sprite_format("").is_err());
    }

    #[test]
    fn test_parse_json_format() {
        assert_eq!(parse_json_format("array"), Ok(JsonFormat::Array));
        assert_eq!(parse_json_format("HASH"), Ok(JsonFormat::Hash));
        assert!(parse_json_format("object").is_err());
    }

    #[test]
    fn test_parse_resize_algorithm() {
        for (name, filter) in [
            ("nearest", FilterType::Nearest),
            ("Triangle", FilterType::Triangle),
            ("gaussian", FilterType::Gaussian),
            ("lanczos3", FilterType::Lanczos3),
            ("catmullrom", FilterType::CatmullRom),
            ("catmull-rom", FilterType::CatmullRom),
        ] {
            assert_eq!(parse_resize_algorithm(name), Ok(filter), "{name}");
        }
        // The help text used to say "nearesr", which gave CatmullRom.
        assert!(parse_resize_algorithm("nearesr").is_err());
    }

    #[test]
    fn test_parse_ordering_bool_and_number() {
        assert_eq!(parse_ordering("-Area"), Ok(Ordering::AreaReverse));
        assert!(parse_ordering("size").is_err());
        assert_eq!(parse_bool("On"), Ok(true));
        assert_eq!(parse_bool("0"), Ok(false));
        assert!(parse_bool("maybe").is_err());
        assert_eq!(parse_number("32"), Ok(32));
        assert!(parse_number("-1").is_err());
        assert!(parse_number("wide").is_err());
    }

    #[test]
    fn test_invalid_value_message_lists_accepted_values() {
        let message = parse_sprite_format("gif").unwrap_err().to_string();
        assert_eq!(
            message,
            "invalid image format 'gif', expected png, webp or avif"
        );
        let message = parse_algorithm("spiral").unwrap_err().to_string();
        assert!(message.starts_with("invalid algorithm 'spiral', expected square"));
    }

    #[test]
    fn test_invalid_env_and_file_values_keep_lower_layers() {
        // An invalid value is ignored (with a warning), so the value from the layer below
        // (here spriters.toml, then the default) stays.
        let config = config_from_env(&[
            ("SPRITERS_FORMAT", "gif"),
            ("SPRITERS_JSON_FORMAT", "object"),
            ("SPRITERS_MAX_WIDTH", "wide"),
            ("SPRITERS_ORDERING", "size"),
        ]);
        let default = Config::default();
        assert_eq!(config.sprite_format, default.sprite_format);
        assert_eq!(config.json_format, default.json_format);
        assert_eq!(config.max_width, None);
        assert_eq!(config.ordering, default.ordering);

        let dir = source_with_toml(
            "invalid",
            "sprite_format = \"webp\"\njson_format = \"hash\"\n",
        );
        let config = resolve_with(
            &dir,
            |_| {},
            &[
                ("SPRITERS_FORMAT", "gif"),
                ("SPRITERS_JSON_FORMAT", "object"),
            ],
        );
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(config.sprite_format, SpriteFormat::WebP);
        assert_eq!(config.json_format, JsonFormat::Hash);

        let mut config = Config::default();
        apply_file_config(
            &mut config,
            FileConfig {
                sprite_format: Some(String::from("gif")),
                algorithm: Some(String::from("spiral")),
                json_format: Some(String::from("hash")),
                ..Default::default()
            },
        );
        assert_eq!(config.sprite_format, SpriteFormat::Png);
        assert_eq!(config.algorithm, Algorithm::Square);
        assert_eq!(config.json_format, JsonFormat::Hash);
    }

    /// Apply `vars` as if they were the process environment.
    fn config_from_env(vars: &[(&str, &str)]) -> Config {
        let vars: std::collections::HashMap<&str, &str> = vars.iter().copied().collect();
        let mut config = Config::default();
        apply_env_with(&mut config, |key| {
            vars.get(key).map(|value| value.to_string())
        });
        config
    }

    #[test]
    fn test_apply_env_vars_algorithm() {
        let config = config_from_env(&[("SPRITERS_ALGORITHM", "vertical")]);
        assert_eq!(config.algorithm, Algorithm::Vertical);
    }

    #[test]
    fn test_apply_env_vars_invalid_algorithm_is_ignored() {
        let config = config_from_env(&[("SPRITERS_ALGORITHM", "bogus")]);
        assert_eq!(config.algorithm, Config::default().algorithm);
    }

    #[test]
    fn test_apply_env_vars_max_dimensions() {
        let config = config_from_env(&[
            ("SPRITERS_MAX_WIDTH", "800"),
            ("SPRITERS_MAX_HEIGHT", "600"),
        ]);
        assert_eq!(config.max_width, Some(800));
        assert_eq!(config.max_height, Some(600));
    }

    #[test]
    fn test_apply_env_vars_cachebuster_flags() {
        let config = config_from_env(&[
            ("SPRITERS_CACHEBUSTER", "true"),
            ("SPRITERS_CACHEBUSTER_FILENAME", "yes"),
        ]);
        assert!(config.cachebuster);
        assert!(config.cachebuster_filename);
    }

    #[test]
    fn test_apply_env_vars_empty_lookup_keeps_defaults() {
        let config = config_from_env(&[]);
        let default = Config::default();
        assert_eq!(config.algorithm, default.algorithm);
        assert_eq!(config.separator, default.separator);
        assert_eq!(config.exclude, default.exclude);
        assert!(!config.force);
    }

    #[test]
    fn test_apply_env_vars_strings_and_lists() {
        let config = config_from_env(&[
            ("SPRITERS_NAMESPACE", "env-ns"),
            ("SPRITERS_URL", "/static/"),
            ("SPRITERS_EXCLUDE", " *.tmp , ,draft-*"),
            ("SPRITERS_FORMAT", "webp"),
            ("SPRITERS_JSON_FORMAT", "HASH"),
        ]);
        assert_eq!(config.namespace.as_deref(), Some("env-ns"));
        assert_eq!(config.url.as_deref(), Some("/static/"));
        assert_eq!(config.exclude, vec!["*.tmp", "draft-*"]);
        assert_eq!(config.sprite_format, SpriteFormat::WebP);
        assert_eq!(config.json_format, crate::JsonFormat::Hash);
    }

    #[test]
    fn test_apply_env_vars_invalid_bool_is_ignored() {
        let config = config_from_env(&[("SPRITERS_CROP", "maybe"), ("SPRITERS_FORCE", "on")]);
        assert!(!config.crop);
        assert!(config.force);
    }

    /// Temporary source directory holding `toml` as its `spriters.toml`.
    fn source_with_toml(tag: &str, toml: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spriters_resolve_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("spriters.toml"), toml).unwrap();
        dir
    }

    fn resolve_with(
        source: &std::path::Path,
        cli: impl Fn(&mut Config),
        env: &[(&str, &str)],
    ) -> Config {
        let env: std::collections::HashMap<&str, &str> = env.iter().copied().collect();
        let source = source.to_path_buf();
        resolve(
            |config| {
                config.source = source.clone();
                cli(config);
            },
            |key| env.get(key).map(|value| value.to_string()),
        )
    }

    #[test]
    fn test_resolve_file_overrides_defaults() {
        let dir = source_with_toml("file", "namespace = \"toml-ns\"\ncrop = true\n");
        let config = resolve_with(&dir, |_| {}, &[]);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(config.namespace.as_deref(), Some("toml-ns"));
        assert!(config.crop);
    }

    #[test]
    fn test_resolve_env_overrides_file() {
        let dir = source_with_toml("env", "namespace = \"toml-ns\"\nforce = true\n");
        let config = resolve_with(
            &dir,
            |_| {},
            &[("SPRITERS_NAMESPACE", "env-ns"), ("SPRITERS_FORCE", "0")],
        );
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(config.namespace.as_deref(), Some("env-ns"));
        assert!(!config.force);
    }

    #[test]
    fn test_resolve_cli_overrides_env_and_file() {
        // Review 3.3: `SPRITERS_FORCE=0 spriters --force` must regenerate.
        let dir = source_with_toml("cli", "namespace = \"toml-ns\"\nforce = false\n");
        let config = resolve_with(
            &dir,
            |config| {
                config.force = true;
                config.namespace = Some(String::from("cli-ns"));
            },
            &[("SPRITERS_NAMESPACE", "env-ns"), ("SPRITERS_FORCE", "0")],
        );
        std::fs::remove_dir_all(&dir).ok();
        assert!(config.force);
        assert_eq!(config.namespace.as_deref(), Some("cli-ns"));
    }

    #[test]
    fn test_resolve_layers_combine_unrelated_options() {
        let dir = source_with_toml("mix", "algorithm = \"diagonal\"\n");
        let config = resolve_with(
            &dir,
            |config| config.json = true,
            &[("SPRITERS_ORDERING", "area")],
        );
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(config.algorithm, Algorithm::Diagonal);
        assert_eq!(config.ordering, Ordering::Area);
        assert!(config.json);
    }

    #[test]
    fn test_resolve_reads_toml_from_cli_source() {
        let dir = source_with_toml("src", "namespace = \"from-cli-source\"\n");
        let config = resolve_with(&dir, |_| {}, &[]);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(config.source, dir);
        assert_eq!(config.namespace.as_deref(), Some("from-cli-source"));
    }

    #[test]
    fn test_resolve_exclude_accumulates() {
        let dir = source_with_toml("excl", "exclude = [\"*.psd\"]\n");
        let config = resolve_with(
            &dir,
            |config| config.exclude.push(String::from("*.tmp")),
            &[("SPRITERS_EXCLUDE", "draft-*")],
        );
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(config.exclude, vec!["*.psd", "draft-*", "*.tmp"]);
    }

    #[test]
    fn test_load_file_config_malformed_toml() {
        let tmp = std::env::temp_dir().join(format!("spriters_bad_toml_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("spriters.toml"), "not valid toml [[[").unwrap();
        let result = load_file_config(&tmp);
        std::fs::remove_dir_all(&tmp).ok();
        assert!(result.is_none(), "malformed TOML must return None");
    }

    #[test]
    fn test_load_file_config_with_sprite_format() {
        let tmp = std::env::temp_dir().join(format!("spriters_fmt_cfg_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("spriters.toml"), "sprite_format = \"webp\"\n").unwrap();
        let fc = load_file_config(&tmp).unwrap();
        std::fs::remove_dir_all(&tmp).ok();
        assert_eq!(fc.sprite_format.as_deref(), Some("webp"));
    }

    #[test]
    fn test_apply_file_config_sprite_format() {
        let tmp = std::env::temp_dir().join(format!("spriters_fc_fmt_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("spriters.toml"), "sprite_format = \"webp\"\n").unwrap();
        let fc = load_file_config(&tmp).unwrap();
        let mut config = Config::default();
        apply_file_config(&mut config, fc);
        std::fs::remove_dir_all(&tmp).ok();
        assert_eq!(config.sprite_format, SpriteFormat::WebP);
    }
}
