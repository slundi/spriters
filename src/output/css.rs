//! Built-in stylesheet outputs: CSS (also used for LESS), SCSS and CSS custom properties.

use std::io::Write;

use super::classes::{build_class_name, entries, entries_by_image, selectors};
use super::names::{get_sprite_url, has_ratios, sprite_file_url, sprite_filename_at};
use crate::{Config, Sprite};

/// CSS `background-position` component for the position `offset`: `0` or `-<offset>px`.
fn css_offset(offset: u32) -> String {
    if offset == 0 {
        String::from("0")
    } else {
        format!("-{offset}px")
    }
}

/// `background-size` declaration of the base rule when the sprite has ratios: every image
/// is then displayed at the 1× sprite size. Empty without ratios.
fn background_size(sprite: &Sprite) -> String {
    if has_ratios(sprite) {
        let (width, height) = sprite.dimensions();
        format!("\n  background-size: {width}px {height}px;")
    } else {
        String::new()
    }
}

/// Media queries switching `selectors` to the image of each ratio above the smallest one
/// (`background-size` set in the base rule keeps them at the 1× size).
fn write_ratio_media_queries(
    f: &mut dyn Write,
    config: &Config,
    sprite: &Sprite,
    selectors: &str,
) -> std::io::Result<()> {
    for &ratio in sprite.ratios.iter().skip(1) {
        let url = sprite_file_url(config, sprite, sprite_filename_at(config, sprite, ratio));
        writeln!(
            f,
            "\n@media screen and (-webkit-min-device-pixel-ratio: {r}), screen and (min--moz-device-pixel-ratio: {r}), screen and (-o-min-device-pixel-ratio: {frac}), screen and (min-device-pixel-ratio: {r}), screen and (min-resolution: {r}dppx) {{",
            r = ratio,
            frac = crate::util::nearest_fration(ratio),
        )?;
        writeln!(
            f,
            "  {selectors} {{\n    background-image: url('{url}');\n  }}\n}}"
        )?;
    }
    Ok(())
}

/// The built-in CSS output; the LESS one is identical.
pub(super) fn generate_css(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let sprite_url = get_sprite_url(config, sprite);
    let entries = entries(config, sprite);
    let all_selectors = selectors(&entries, ", ");

    // Write the shared class for all images
    writeln!(
        f,
        "{all_selectors} {{\n  background-image: url('{sprite_url}');\n  background-repeat: no-repeat;{size}\n}}",
        size = background_size(sprite),
    )?;

    // Write individual image classes (primary + aliases share the same rule).
    for group in entries_by_image(&entries) {
        let img = group[0].image;
        writeln!(
            f,
            "\n{selector} {{\n  background-position: {x} {y};\n  width: {w}px;\n  height: {h}px;\n}}",
            selector = selectors(group, ", "),
            x = css_offset(img.x),
            y = css_offset(img.y),
            w = img.computed_width,
            h = img.computed_height,
        )?;
    }

    write_ratio_media_queries(f, config, sprite, &all_selectors)?;
    Ok(())
}

pub(super) fn generate_scss(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let name = sprite.name();
    let entries = entries(config, sprite);
    let (width, height) = sprite.dimensions();
    let sprite_url = get_sprite_url(config, sprite);

    // Generate a mixin for the sprite base
    writeln!(
        f,
        "@mixin {name}-sprite {{\n  background-image: url('{url}');\n  background-repeat: no-repeat;\n  background-size: {w}px {h}px;\n}}",
        name = name,
        url = sprite_url,
        w = width,
        h = height,
    )?;

    // Generate the shared class
    writeln!(
        f,
        "\n{list} {{\n  @include {name}-sprite;\n}}",
        list = selectors(&entries, ","),
        name = name,
    )?;

    // Generate individual image classes (primary + aliases share the same rule).
    for group in entries_by_image(&entries) {
        let img = group[0].image;
        writeln!(
            f,
            "\n{selector} {{\n  background-position: -{x}px -{y}px;\n  width: {w}px;\n  height: {h}px;\n}}",
            selector = selectors(group, ", "),
            x = img.x,
            y = img.y,
            w = img.computed_width,
            h = img.computed_height,
        )?;
    }
    // The mixin already sets `background-size`.
    write_ratio_media_queries(f, config, sprite, &selectors(&entries, ", "))?;
    Ok(())
}

pub(super) fn generate_css_vars(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let name = sprite.name();
    let sprite_url = get_sprite_url(config, sprite);

    writeln!(f, ":root {{")?;
    writeln!(
        f,
        "  --{name}-image: url('{url}');",
        name = name,
        url = sprite_url
    )?;

    // Custom property names cannot hold a pseudo-class: the full file label is used.
    for entry in entries(config, sprite) {
        let img = entry.image;
        let prop = build_class_name(config, &name, &entry.label);
        writeln!(f, "  --{prop}-x: {x};", x = css_offset(img.x))?;
        writeln!(f, "  --{prop}-y: {y};", y = css_offset(img.y))?;
        writeln!(f, "  --{prop}-w: {w}px;", w = img.computed_width)?;
        writeln!(f, "  --{prop}-h: {h}px;", h = img.computed_height)?;
    }

    writeln!(f, "}}")?;
    Ok(())
}
