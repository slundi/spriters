//! CSS class names and selectors of the sprite entries.

use crate::labels::{css_label_from_path, label_from_path};
use crate::{Config, ImageAndPosition, Sprite};

/// `--separator` value joining the class name parts in camelCase (see [`camel_case`]).
const CAMEL_CASE: &str = "camelcase";

/// Build a CSS class name based on config settings (namespace, sprite_namespace, separator).
/// Default pattern: `sprite-{sprite_name}-{image_label}`, empty parts left out; in camelCase
/// with `--separator=camelcase`.
pub(super) fn build_class_name(config: &Config, sprite_name: &str, image_label: &str) -> String {
    // Determine namespace (default: "sprite")
    let namespace = config.namespace.as_deref().unwrap_or("sprite");

    // Determine sprite namespace (default: sprite name, supports %(sprite)s placeholder)
    let sprite_ns = match &config.sprite_namespace {
        Some(sn) => sn.replace("%(sprite)s", sprite_name),
        None => sprite_name.to_string(),
    };

    let parts = [namespace, &sprite_ns, image_label];
    if config.separator.eq_ignore_ascii_case(CAMEL_CASE) {
        return camel_case(&parts);
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(&config.separator)
}

/// The words of `parts` in camelCase: `["sprite", "icons", "my-image"]` gives
/// `spriteIconsMyImage`. Every character that is not alphanumeric separates words.
fn camel_case(parts: &[&str]) -> String {
    let mut result = String::new();
    let words = parts
        .iter()
        .flat_map(|part| part.split(|character: char| !character.is_alphanumeric()))
        .filter(|word| !word.is_empty());
    for word in words {
        if result.is_empty() {
            result.push_str(word);
            continue;
        }
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            result.extend(first.to_uppercase());
            result.push_str(characters.as_str());
        }
    }
    result
}

/// An image or one of its aliases, as named in the outputs.
pub(super) struct Entry<'a> {
    pub(super) image: &'a ImageAndPosition,
    /// Source file: the image or the alias.
    pub(super) path: &'a str,
    /// Name in the non-CSS outputs (JSON keys…), see [`label_from_path`].
    pub(super) label: String,
    /// CSS class name, see [`build_class_name`].
    pub(super) class: String,
    /// Pseudo-classes (`:hover`) from `--pseudo-class-separator`, usually empty.
    pub(super) pseudo: String,
}

impl Entry<'_> {
    /// CSS selector: `.class:pseudo`.
    pub(super) fn selector(&self) -> String {
        format!(".{}{}", self.class, self.pseudo)
    }
}

/// Every entry of the sprite: each image followed by its aliases (same position).
pub(super) fn entries<'a>(config: &Config, sprite: &'a Sprite) -> Vec<Entry<'a>> {
    let name = sprite.name();
    let pseudo_separator = config.pseudo_class_separator.as_deref();
    sprite
        .images
        .iter()
        .flat_map(|image| {
            std::iter::once(image.path.as_str())
                .chain(image.aliases.iter().map(String::as_str))
                .map(move |path| (image, path))
        })
        .map(|(image, path)| {
            let (css_label, pseudo) = css_label_from_path(path, pseudo_separator);
            Entry {
                image,
                path,
                label: label_from_path(path),
                class: build_class_name(config, &name, &css_label),
                pseudo,
            }
        })
        .collect()
}

/// The entries grouped by image: an image and its aliases share one CSS rule.
pub(super) fn entries_by_image<'a, 'b>(
    entries: &'b [Entry<'a>],
) -> impl Iterator<Item = &'b [Entry<'a>]> {
    entries.chunk_by(|left, right| std::ptr::eq(left.image, right.image))
}

/// The selectors of `entries`, joined with `separator`.
pub(super) fn selectors(entries: &[Entry], separator: &str) -> String {
    entries
        .iter()
        .map(Entry::selector)
        .collect::<Vec<_>>()
        .join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_class_name() {
        let mut config = Config::default();

        // Default namespace "sprite" with sprite name and image label
        let class_name = build_class_name(&config, "icons", "arrow");
        assert_eq!(class_name, "sprite-icons-arrow");

        // Custom namespace
        config.namespace = Some("my-ns".to_string());
        let class_name = build_class_name(&config, "icons", "arrow");
        assert_eq!(class_name, "my-ns-icons-arrow");

        // Empty namespace
        config.namespace = Some("".to_string());
        let class_name = build_class_name(&config, "icons", "arrow");
        assert_eq!(class_name, "icons-arrow");

        // Custom sprite namespace
        config.namespace = Some("sprite".to_string());
        config.sprite_namespace = Some("custom".to_string());
        let class_name = build_class_name(&config, "icons", "arrow");
        assert_eq!(class_name, "sprite-custom-arrow");

        // Sprite namespace with placeholder
        config.sprite_namespace = Some("%(sprite)s-extra".to_string());
        let class_name = build_class_name(&config, "icons", "arrow");
        assert_eq!(class_name, "sprite-icons-extra-arrow");

        // Custom separator
        config.sprite_namespace = None;
        config.separator = "_".to_string();
        let class_name = build_class_name(&config, "icons", "arrow");
        assert_eq!(class_name, "sprite_icons_arrow");

        // Empty sprite namespace and image label
        config.separator = "-".to_string();
        config.sprite_namespace = Some("".to_string());
        let class_name = build_class_name(&config, "icons", "");
        assert_eq!(class_name, "sprite");
    }

    #[test]
    fn test_camel_case_separator() {
        // Review §7: `--separator=camelcase` was used literally as a separator.
        assert_eq!(
            camel_case(&["sprite", "icons", "my-image"]),
            "spriteIconsMyImage"
        );
        assert_eq!(camel_case(&["", "icons.v2", "a_b--c"]), "iconsV2ABC");
        assert_eq!(camel_case(&["my-ns", "", "été"]), "myNsÉté");
        assert_eq!(camel_case(&["", "", ""]), "");

        for separator in ["camelcase", "camelCase", "CAMELCASE"] {
            let mut config = Config {
                separator: String::from(separator),
                ..Default::default()
            };
            assert_eq!(
                build_class_name(&config, "icons", "arrow-left"),
                "spriteIconsArrowLeft"
            );
            config.namespace = Some(String::new());
            config.sprite_namespace = Some(String::from("%(sprite)s-x"));
            assert_eq!(build_class_name(&config, "icons", "up"), "iconsXUp");
        }
    }
}
