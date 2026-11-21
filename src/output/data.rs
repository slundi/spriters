//! Outputs for other tools: JSON, CAAT, Cocos2d plist, Tailwind safelist and TypeScript
//! definitions.

use std::io::Write;

use super::SPRITERS_VERSION;
use super::classes::entries;
use super::meta::compute_sprite_hash;
use super::names::{get_sprite_url, sprite_filename};
use crate::labels::label_from_path;
use crate::{Config, JsonFormat, Sprite};

pub(super) fn generate_coco2d(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let (width, height) = sprite.dimensions();
    let sprite_url = get_sprite_url(config, sprite);

    // Write plist header
    writeln!(f, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        f,
        r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">"#
    )?;
    writeln!(f, r#"<plist version="1.0">"#)?;
    writeln!(f, "<dict>")?;

    // Write frames (primary + aliases share the same region).
    writeln!(f, "    <key>frames</key>")?;
    writeln!(f, "    <dict>")?;

    for img in sprite.images.iter() {
        let labels: Vec<String> = std::iter::once(img.label())
            .chain(img.aliases.iter().map(|p| label_from_path(p)))
            .collect();
        for lbl in &labels {
            writeln!(f, "        <key>{}</key>", lbl)?;
            writeln!(f, "        <dict>")?;
            writeln!(f, "            <key>frame</key>")?;
            writeln!(
                f,
                "            <string>{{{{{},{}}},{{{},{}}}}}</string>",
                img.x, img.y, img.computed_width, img.computed_height
            )?;
            writeln!(f, "            <key>offset</key>")?;
            writeln!(f, "            <string>{{0,0}}</string>")?;
            writeln!(f, "            <key>rotated</key>")?;
            writeln!(f, "            <false/>")?;
            writeln!(f, "            <key>sourceColorRect</key>")?;
            writeln!(
                f,
                "            <string>{{{{0,0}},{{{},{}}}}}</string>",
                img.computed_width, img.computed_height
            )?;
            writeln!(f, "            <key>sourceSize</key>")?;
            writeln!(
                f,
                "            <string>{{{},{}}}</string>",
                img.computed_width, img.computed_height
            )?;
            writeln!(f, "        </dict>")?;
        }
    }

    writeln!(f, "    </dict>")?;

    // Write metadata
    writeln!(f, "    <key>metadata</key>")?;
    writeln!(f, "    <dict>")?;
    writeln!(f, "        <key>format</key>")?;
    writeln!(f, "        <integer>2</integer>")?;
    writeln!(f, "        <key>size</key>")?;
    writeln!(f, "        <string>{{{},{}}}</string>", width, height)?;
    writeln!(f, "        <key>textureFileName</key>")?;
    writeln!(f, "        <string>{}</string>", sprite_url)?;
    writeln!(f, "    </dict>")?;

    writeln!(f, "</dict>")?;
    writeln!(f, "</plist>")?;
    Ok(())
}

pub(super) fn generate_json(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let name = sprite.name();
    let (width, height) = sprite.dimensions();
    let sprite_url = get_sprite_url(config, sprite);

    // Flatten: primary + aliases all become independent entries at the same coordinates.
    let entries: Vec<(String, u32, u32, u32, u32)> = sprite
        .images
        .iter()
        .flat_map(|img| {
            std::iter::once(img.label())
                .chain(img.aliases.iter().map(|p| label_from_path(p)))
                .map(|lbl| (lbl, img.x, img.y, img.computed_width, img.computed_height))
                .collect::<Vec<_>>()
        })
        .collect();
    let n = entries.len();

    match config.json_format {
        JsonFormat::Array => {
            writeln!(f, "{{\"frames\":[")?;
            for (index, (label, x, y, w, h)) in entries.iter().enumerate() {
                writeln!(
                    f,
                    "{{\"filename\": \"{label}\", \"frame\":{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}}}{coma}",
                    coma = if index + 1 < n { "," } else { "" },
                )?;
            }
            writeln!(
                f,
                "],\n\"meta\":{{\"width\": {w}, \"height\": {h}, \"sprite_filename\": \"{name}\", \"sprite_url\": \"{url}\"}}\n}}",
                w = width,
                h = height,
                url = sprite_url,
            )?;
        }
        JsonFormat::Hash => {
            writeln!(f, "{{")?;
            for (index, (label, x, y, w, h)) in entries.iter().enumerate() {
                writeln!(
                    f,
                    "\"{label}\": {{\"x\": {x}, \"y\": {y}, \"w\": {w}, \"h\": {h}}}{coma}",
                    coma = if index + 1 < n { "," } else { "" },
                )?;
            }
            writeln!(f, "}}")?;
        }
    }
    Ok(())
}

pub(super) fn generate_caat(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let (width, height) = sprite.dimensions();
    let sprite_filename = sprite_filename(config, sprite);
    let hash = compute_sprite_hash(config, sprite);

    // Flatten primary + alias entries (CAAT uses original filenames, not labels).
    let entries: Vec<(String, u32, u32, u32, u32)> = sprite
        .images
        .iter()
        .flat_map(|img| {
            let primary_filename = std::path::Path::new(&img.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            std::iter::once(primary_filename)
                .chain(img.aliases.iter().map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                }))
                .map(|fn_| (fn_, img.x, img.y, img.computed_width, img.computed_height))
                .collect::<Vec<_>>()
        })
        .collect();
    let n = entries.len();

    writeln!(f, "{{")?;
    writeln!(f, "    \"sprites\": {{")?;
    for (index, (filename, x, y, w, h)) in entries.iter().enumerate() {
        writeln!(
            f,
            "        \"{fn}\": {{\"x\": {x}, \"y\": {y}, \"width\": {w}, \"height\": {h}}}{coma}",
            fn = filename, x = x, y = y, w = w, h = h,
            coma = if index + 1 < n { "," } else { "" },
        )?;
    }
    writeln!(f, "    }},")?;
    writeln!(f, "    \"meta\": {{")?;
    writeln!(f, "        \"version\": \"{}\",", SPRITERS_VERSION)?;
    writeln!(f, "        \"hash\": \"{:x}\",", hash)?;
    writeln!(f, "        \"sprite_filename\": \"{}\",", sprite_filename)?;
    writeln!(f, "        \"width\": {},", width)?;
    writeln!(f, "        \"height\": {}", height)?;
    writeln!(f, "    }}")?;
    writeln!(f, "}}")?;
    Ok(())
}

pub(super) fn generate_tailwind(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    // Classes only: `button` and `button:hover` share one.
    let mut class_names: Vec<String> = Vec::new();
    for entry in entries(config, sprite) {
        if !class_names.contains(&entry.class) {
            class_names.push(entry.class);
        }
    }

    let n = class_names.len();
    writeln!(f, "{{")?;
    writeln!(f, "  \"safelist\": [")?;
    for (i, cls) in class_names.iter().enumerate() {
        let comma = if i + 1 < n { "," } else { "" };
        writeln!(f, "    \"{}\"{}", cls, comma)?;
    }
    writeln!(f, "  ]")?;
    writeln!(f, "}}")?;
    Ok(())
}

pub(super) fn generate_dts(
    _config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let labels: Vec<String> = sprite
        .images
        .iter()
        .flat_map(|img| {
            std::iter::once(img.label())
                .chain(img.aliases.iter().map(|p| label_from_path(p)))
                .collect::<Vec<_>>()
        })
        .collect();

    if labels.is_empty() {
        writeln!(f, "export type SpriteName = never;")?;
        return Ok(());
    }

    write!(f, "export type SpriteName")?;
    for (i, lbl) in labels.iter().enumerate() {
        let semi = if i + 1 == labels.len() { ";" } else { "" };
        writeln!(f, "\n  | \"{}\"{}", lbl, semi)?;
    }
    Ok(())
}
