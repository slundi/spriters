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
    if let Some(v) = lookup("SPRITERS_ALGORITHM")
        && let Ok(a) = parse_algorithm(&v)
    {
        config.algorithm = a;
    }
    if let Some(v) = lookup("SPRITERS_ORDERING")
        && let Ok(o) = v.parse::<Ordering>()
    {
        config.ordering = o;
    }
    if let Some(v) = lookup("SPRITERS_CROP")
        && let Ok(b) = parse_bool(&v)
    {
        config.crop = b;
    }
    if let Some(v) = lookup("SPRITERS_MAX_WIDTH")
        && let Ok(n) = v.parse::<u32>()
    {
        config.max_width = Some(n);
    }
    if let Some(v) = lookup("SPRITERS_MAX_HEIGHT")
        && let Ok(n) = v.parse::<u32>()
    {
        config.max_height = Some(n);
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
    if let Some(v) = lookup("SPRITERS_JSON_FORMAT") {
        config.json_format = if v.to_lowercase() == "hash" {
            JsonFormat::Hash
        } else {
            JsonFormat::Array
        };
    }
    if let Some(v) = lookup("SPRITERS_CACHEBUSTER")
        && let Ok(b) = parse_bool(&v)
    {
        config.cachebuster = b;
    }
    if let Some(v) = lookup("SPRITERS_CACHEBUSTER_FILENAME")
        && let Ok(b) = parse_bool(&v)
    {
        config.cachebuster_filename = b;
    }
    if let Some(v) = lookup("SPRITERS_CACHEBUSTER_FILENAME_ONLY_SPRITES")
        && let Ok(b) = parse_bool(&v)
    {
        config.cachebuster_filename_only_sprites = b;
    }
    if let Some(v) = lookup("SPRITERS_FORCE")
        && let Ok(b) = parse_bool(&v)
    {
        config.force = b;
    }
    if let Some(v) = lookup("SPRITERS_FOLLOW_LINKS")
        && let Ok(b) = parse_bool(&v)
    {
        config.follow_links = b;
    }
    if let Some(v) = lookup("SPRITERS_RECURSIVE")
        && let Ok(b) = parse_bool(&v)
    {
        config.recursive = b;
    }
    if let Some(v) = lookup("SPRITERS_PROJECT")
        && let Ok(b) = parse_bool(&v)
    {
        config.project = b;
    }
    if let Some(v) = lookup("SPRITERS_NO_CSS")
        && let Ok(b) = parse_bool(&v)
    {
        config.no_css = b;
    }
    if let Some(v) = lookup("SPRITERS_NO_IMG")
        && let Ok(b) = parse_bool(&v)
    {
        config.no_img = b;
    }
    if let Some(v) = lookup("SPRITERS_PNG8")
        && let Ok(b) = parse_bool(&v)
    {
        config.png8 = b;
    }
    if let Some(v) = lookup("SPRITERS_HTML")
        && let Ok(b) = parse_bool(&v)
    {
        config.html = b;
    }
    if let Some(v) = lookup("SPRITERS_JSON")
        && let Ok(b) = parse_bool(&v)
    {
        config.json = b;
    }
    if let Some(v) = lookup("SPRITERS_CAAT")
        && let Ok(b) = parse_bool(&v)
    {
        config.caat = b;
    }
    if let Some(v) = lookup("SPRITERS_LESS")
        && let Ok(b) = parse_bool(&v)
    {
        config.less = b;
    }
    if let Some(v) = lookup("SPRITERS_SCSS")
        && let Ok(b) = parse_bool(&v)
    {
        config.scss = b;
    }
    if let Some(v) = lookup("SPRITERS_COCOS2D")
        && let Ok(b) = parse_bool(&v)
    {
        config.cocos2d = b;
    }
    if let Some(v) = lookup("SPRITERS_NO_DUPLICATE")
        && let Ok(b) = parse_bool(&v)
    {
        config.no_duplicate = b;
    }
    if let Some(v) = lookup("SPRITERS_PSEUDO_CLASS_SEPARATOR") {
        config.pseudo_class_separator = Some(v);
    }
    if let Some(v) = lookup("SPRITERS_FORMAT") {
        config.sprite_format = parse_sprite_format(&v);
    }
    if let Some(v) = lookup("SPRITERS_EXCLUDE") {
        config.exclude.extend(
            v.split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
        );
    }
    if let Some(v) = lookup("SPRITERS_CSS_VARS")
        && let Ok(b) = parse_bool(&v)
    {
        config.css_vars = b;
    }
    if let Some(v) = lookup("SPRITERS_DRY_RUN")
        && let Ok(b) = parse_bool(&v)
    {
        config.dry_run = b;
    }
    if let Some(v) = lookup("SPRITERS_CHECK")
        && let Ok(b) = parse_bool(&v)
    {
        config.check = b;
    }
    if let Some(v) = lookup("SPRITERS_TAILWIND")
        && let Ok(b) = parse_bool(&v)
    {
        config.tailwind = b;
    }
    if let Some(v) = lookup("SPRITERS_DTS")
        && let Ok(b) = parse_bool(&v)
    {
        config.dts = b;
    }
}

/// Overlay file config values onto config. Lowest priority after the defaults: env vars
/// and the command line override it (see [`resolve`]).
pub fn apply_file_config(config: &mut Config, fc: FileConfig) {
    if let Some(v) = fc.algorithm
        && let Ok(a) = parse_algorithm(&v)
    {
        config.algorithm = a;
    }
    if let Some(v) = fc.ordering
        && let Ok(o) = v.parse::<Ordering>()
    {
        config.ordering = o;
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
        assert_eq!(parse_sprite_format("webp"), SpriteFormat::WebP);
        assert_eq!(parse_sprite_format("WEBP"), SpriteFormat::WebP);
        assert_eq!(parse_sprite_format("avif"), SpriteFormat::Avif);
        assert_eq!(parse_sprite_format("AVIF"), SpriteFormat::Avif);
        assert_eq!(parse_sprite_format("png"), SpriteFormat::Png);
        assert_eq!(parse_sprite_format("unknown"), SpriteFormat::Png);
        assert_eq!(parse_sprite_format(""), SpriteFormat::Png);
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
