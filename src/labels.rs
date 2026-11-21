//! Names of the images in the outputs (labels, pseudo-classes), and detection of the images
//! whose names collide.

use std::collections::HashMap;
use std::path::Path;

use log::{debug, error};

use crate::ImageAndPosition;

/// CSS-safe label of the image file `path`: its slugified file stem. Used for the image
/// itself and for its aliases.
pub(crate) fn label_from_path(path: &str) -> String {
    slug::slugify(
        std::path::Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
    )
}

/// Pseudo-classes recognised at the end of a file name with `--pseudo-class-separator`.
pub(crate) const CSS_PSEUDO_CLASSES: [&str; 10] = [
    "link",
    "visited",
    "active",
    "hover",
    "focus",
    "first-letter",
    "first-line",
    "first-child",
    "before",
    "after",
];

/// CSS label and pseudo-classes of the image file `path`. With a pseudo-class `separator`,
/// known pseudo-classes are taken from the end of the file stem before it is slugified:
/// with `__`, `button__hover.png` gives `("button", ":hover")` and `link__focus__hover.png`
/// gives `("link", ":focus:hover")`. Otherwise the label is [`label_from_path`].
pub(crate) fn css_label_from_path(path: &str, separator: Option<&str>) -> (String, String) {
    let stem = Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let mut base: &str = &stem;
    let mut pseudo_classes = Vec::new();
    if let Some(separator) = separator.filter(|separator| !separator.is_empty()) {
        while let Some((rest, suffix)) = base.rsplit_once(separator) {
            let suffix = suffix.to_ascii_lowercase();
            if rest.is_empty() || !CSS_PSEUDO_CLASSES.contains(&suffix.as_str()) {
                break;
            }
            pseudo_classes.push(suffix);
            base = rest;
        }
    }
    let pseudo = pseudo_classes
        .iter()
        .rev()
        .map(|pseudo_class| format!(":{pseudo_class}"))
        .collect();
    (slug::slugify(base), pseudo)
}

/// The names under which `path` appears in the outputs: its label (JSON keys, class names),
/// plus its CSS selector key `label:pseudo` when it has pseudo-classes. The latter never
/// equals a label, which cannot contain `:`.
fn label_keys(path: &str, pseudo_separator: Option<&str>) -> Vec<String> {
    let mut keys = vec![label_from_path(path)];
    let (label, pseudo) = css_label_from_path(path, pseudo_separator);
    if !pseudo.is_empty() {
        keys.push(format!("{label}{pseudo}"));
    }
    keys
}

/// Leave out the images and aliases whose label is already taken by another image: both
/// would get the same class name (and JSON key) at different positions, e.g. `my_image.png`
/// and `my-image.png`, or `rss.png` in two folders with `--recursive`. With a
/// `pseudo_separator`, CSS selectors (`button:hover`) must be unique too (see
/// [`label_keys`]).
///
/// Images claim their label first, in path order, so the same one is kept whatever the walk
/// order; then aliases. An alias with the label of its own image adds nothing and is dropped
/// silently. Returns the paths left out.
pub(crate) fn remove_label_collisions(
    images: &mut Vec<ImageAndPosition>,
    pseudo_separator: Option<&str>,
) -> Vec<String> {
    images.sort_by(|a, b| a.path.cmp(&b.path));
    // Label -> path of the image whose position it designates.
    let mut owners: HashMap<String, String> = HashMap::new();
    let mut left_out = Vec::new();

    images.retain(|image| {
        let keys = label_keys(&image.path, pseudo_separator);
        if let Some((key, owner)) = keys
            .iter()
            .find_map(|key| owners.get(key).map(|owner| (key, owner)))
        {
            error!(
                "{} is left out: its label '{}' is already used by {}, rename one of them",
                image.path, key, owner
            );
            left_out.push(image.path.clone());
            left_out.extend(image.aliases.iter().cloned());
            return false;
        }
        for key in keys {
            owners.insert(key, image.path.clone());
        }
        true
    });

    for image in images.iter_mut() {
        image.aliases.retain(|alias| {
            let keys = label_keys(alias, pseudo_separator);
            let taken = keys
                .iter()
                .find_map(|key| owners.get(key).map(|owner| (key, owner)));
            match taken {
                Some((_, owner)) if *owner == image.path => {
                    debug!("{alias}: same label as {}, nothing to add", image.path);
                    false
                }
                Some((key, owner)) => {
                    error!(
                        "{alias} is left out: its label '{key}' is already used by {owner}, rename one of them"
                    );
                    left_out.push(alias.clone());
                    false
                }
                None => {
                    for key in keys {
                        owners.insert(key, image.path.clone());
                    }
                    true
                }
            }
        });
    }
    left_out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Image `path` with `aliases`, for the label collision tests.
    fn labelled(path: &str, aliases: &[&str]) -> ImageAndPosition {
        ImageAndPosition {
            path: String::from(path),
            aliases: aliases.iter().map(|alias| String::from(*alias)).collect(),
            ..Default::default()
        }
    }

    fn paths(images: &[ImageAndPosition]) -> Vec<(&str, Vec<&str>)> {
        images
            .iter()
            .map(|image| {
                let aliases = image.aliases.iter().map(String::as_str).collect();
                (image.path.as_str(), aliases)
            })
            .collect()
    }

    #[test]
    fn test_label_collisions_keep_the_first_path() {
        // Review 6.4: both images got the class `my-image` at different positions.
        // Given in reverse order: the kept image must not depend on the walk order.
        let mut images = vec![
            labelled("icons/rss.png", &[]),
            labelled("icons/my_image.png", &[]),
            labelled("icons/blog/rss.png", &[]),
            labelled("icons/my-image.png", &[]),
            labelled("icons/other.png", &[]),
        ];
        let left_out = remove_label_collisions(&mut images, None);

        assert_eq!(
            paths(&images),
            [
                ("icons/blog/rss.png", vec![]),
                ("icons/my-image.png", vec![]),
                ("icons/other.png", vec![]),
            ]
        );
        assert_eq!(left_out, ["icons/my_image.png", "icons/rss.png"]);
    }

    #[test]
    fn test_label_collisions_with_aliases() {
        let mut images = vec![
            // Same content in two folders: same label, same position.
            labelled("icons/rss.png", &["icons/blog/rss.png", "icons/feed.png"]),
            // The alias `home` collides with an image: images claim labels first.
            labelled("icons/house.png", &["icons/old/home.png"]),
            labelled("icons/home.png", &[]),
            // The alias `feed` was claimed by the alias of `rss.png`.
            labelled("icons/stream.png", &["icons/blog/feed.png"]),
            // A left-out image takes its aliases with it.
            labelled("icons/zz/other.png", &["icons/zz/other-copy.png"]),
            labelled("icons/other.png", &[]),
        ];
        let mut left_out = remove_label_collisions(&mut images, None);
        left_out.sort();

        assert_eq!(
            paths(&images),
            [
                ("icons/home.png", vec![]),
                ("icons/house.png", vec![]),
                ("icons/other.png", vec![]),
                ("icons/rss.png", vec!["icons/feed.png"]),
                ("icons/stream.png", vec![]),
            ]
        );
        assert_eq!(
            left_out,
            [
                "icons/blog/feed.png",
                "icons/old/home.png",
                "icons/zz/other-copy.png",
                "icons/zz/other.png",
            ]
        );
    }

    #[test]
    fn test_unique_labels_are_untouched() {
        let mut images = vec![labelled("b.png", &["b-copy.png"]), labelled("a.png", &[])];
        assert!(remove_label_collisions(&mut images, None).is_empty());
        assert_eq!(
            paths(&images),
            [("a.png", vec![]), ("b.png", vec!["b-copy.png"])]
        );
    }

    #[test]
    fn test_css_label_from_path() {
        // Review §7: `--pseudo-class-separator` was ignored, and slugify turned
        // `button__hover` into `button-hover`.
        let separator = Some("__");
        for (path, label, pseudo) in [
            ("icons/button__hover.png", "button", ":hover"),
            ("icons/my_link__focus__HOVER.png", "my-link", ":focus:hover"),
            ("icons/arrow__first-child.png", "arrow", ":first-child"),
            // Not a pseudo-class: part of the label.
            ("icons/menu__open.png", "menu-open", ""),
            ("icons/menu__open__hover.png", "menu-open", ":hover"),
            // A pseudo-class alone is a label.
            ("icons/__hover.png", "hover", ""),
            ("icons/hover.png", "hover", ""),
            ("icons/plain.png", "plain", ""),
        ] {
            assert_eq!(
                css_label_from_path(path, separator),
                (String::from(label), String::from(pseudo)),
                "{path}"
            );
        }
        // Without separator (the default), or an empty one, the file stem is the label.
        for separator in [None, Some("")] {
            assert_eq!(
                css_label_from_path("icons/button__hover.png", separator),
                (String::from("button-hover"), String::new())
            );
        }
        assert_eq!(
            css_label_from_path("icons/nav-active.png", Some("-")),
            (String::from("nav"), String::from(":active"))
        );
    }

    #[test]
    fn test_label_collisions_with_pseudo_classes() {
        let mut images = vec![
            labelled("icons/button.png", &[]),
            labelled("icons/button__hover.png", &[]),
            // Same selector `.button:hover`.
            labelled("icons/zz/button__hover.png", &[]),
            // Same JSON key `button-hover`.
            labelled("icons/zz/button-hover.png", &[]),
            labelled("icons/link__focus.png", &["icons/link__active.png"]),
        ];
        let mut left_out = remove_label_collisions(&mut images, Some("__"));
        left_out.sort();
        assert_eq!(
            paths(&images),
            [
                ("icons/button.png", vec![]),
                ("icons/button__hover.png", vec![]),
                ("icons/link__focus.png", vec!["icons/link__active.png"]),
            ]
        );
        assert_eq!(
            left_out,
            ["icons/zz/button-hover.png", "icons/zz/button__hover.png"]
        );
    }

    #[test]
    fn test_css_pseudo_classes_constant() {
        // Verify known pseudo classes are present
        assert!(CSS_PSEUDO_CLASSES.contains(&"hover"));
        assert!(CSS_PSEUDO_CLASSES.contains(&"active"));
        assert!(CSS_PSEUDO_CLASSES.contains(&"focus"));
        assert_eq!(CSS_PSEUDO_CLASSES.len(), 10);
    }
}
