use image::{GenericImage, RgbaImage};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use crate::{
    Config, ImageAndPosition, JsonFormat, Sprite, SpriteFormat, css_label_from_path,
    label_from_path,
};

const SPRITERS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `--separator` value joining the class name parts in camelCase (see [`camel_case`]).
const CAMEL_CASE: &str = "camelcase";

/// Deterministic cache buster: lower 32 bits of the sprite hash, hex-encoded.
fn cache_buster(config: &Config, sprite: &Sprite) -> String {
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
fn has_ratios(sprite: &Sprite) -> bool {
    sprite.ratios.len() > 1 || base_ratio(sprite) != 1.0
}

/// File name of the sprite image at `ratio`: `name[_<buster>][@<ratio>x].ext`, the buster
/// being added with `--cachebuster-filename` / `--cachebuster-filename-only-sprites`.
///
/// Single source of truth for the written files and every reference to them.
fn sprite_filename_at(config: &Config, sprite: &Sprite, ratio: f32) -> String {
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
pub(crate) fn sprite_filename(config: &Config, sprite: &Sprite) -> String {
    sprite_filename_at(config, sprite, base_ratio(sprite))
}

/// File names of every sprite image, one per ratio, smallest ratio first.
fn sprite_filenames(config: &Config, sprite: &Sprite) -> Vec<String> {
    sprite
        .ratios
        .iter()
        .map(|&ratio| sprite_filename_at(config, sprite, ratio))
        .collect()
}

/// Get the sprite image URL for use in CSS.
/// - If `config.url` is set, uses that as base URL
/// - Cache buster is derived deterministically from the sprite content hash.
fn get_sprite_url(config: &Config, sprite: &Sprite) -> String {
    sprite_file_url(config, sprite, sprite_filename(config, sprite))
}

/// URL of the sprite image file `filename`, see [`get_sprite_url`].
fn sprite_file_url(config: &Config, sprite: &Sprite, mut filename: String) -> String {
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

/// Build a CSS class name based on config settings (namespace, sprite_namespace, separator).
/// Default pattern: `sprite-{sprite_name}-{image_label}`, empty parts left out; in camelCase
/// with `--separator=camelcase`.
fn build_class_name(config: &Config, sprite_name: &str, image_label: &str) -> String {
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
struct Entry<'a> {
    image: &'a ImageAndPosition,
    /// Source file: the image or the alias.
    path: &'a str,
    /// Name in the non-CSS outputs (JSON keys…), see [`label_from_path`].
    label: String,
    /// CSS class name, see [`build_class_name`].
    class: String,
    /// Pseudo-classes (`:hover`) from `--pseudo-class-separator`, usually empty.
    pseudo: String,
}

impl Entry<'_> {
    /// CSS selector: `.class:pseudo`.
    fn selector(&self) -> String {
        format!(".{}{}", self.class, self.pseudo)
    }
}

/// Every entry of the sprite: each image followed by its aliases (same position).
fn entries<'a>(config: &Config, sprite: &'a Sprite) -> Vec<Entry<'a>> {
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
fn entries_by_image<'a, 'b>(entries: &'b [Entry<'a>]) -> impl Iterator<Item = &'b [Entry<'a>]> {
    entries.chunk_by(|left, right| std::ptr::eq(left.image, right.image))
}

/// The selectors of `entries`, joined with `separator`.
fn selectors(entries: &[Entry], separator: &str) -> String {
    entries
        .iter()
        .map(Entry::selector)
        .collect::<Vec<_>>()
        .join(separator)
}

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

/// Temporary file `.<file name>.<pid>.tmp` in which `path` is written before being renamed
/// (see [`replace_file`]): hidden, in the same directory so that the rename stays on one
/// file system, and unique per process so that concurrent runs do not mix their content.
fn temp_path(path: &std::path::Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

/// Name of the file whose temporary file (see [`temp_path`]) is `file_name`, if it is one.
fn temp_target(file_name: &str) -> Option<&str> {
    let (target, pid) = file_name
        .strip_prefix('.')?
        .strip_suffix(".tmp")?
        .rsplit_once('.')?;
    (!target.is_empty() && !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
        .then_some(target)
}

/// Write `file` through a buffer with `write`, then flush it explicitly: dropping a
/// `BufWriter` ignores the error of its last flush (e.g. a full disk).
fn write_buffered(
    file: File,
    write: impl FnOnce(&mut BufWriter<File>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(file);
    write(&mut writer)?;
    writer.flush()
}

/// Replace `path` atomically: the content is written to a temporary file next to it (see
/// [`temp_path`]), renamed over `path` once complete. A crash or a write error never leaves
/// a truncated file: `path` keeps its previous content, and the temporary file is removed.
fn replace_file(
    path: &std::path::Path,
    write: impl FnOnce(&mut BufWriter<File>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let temp = temp_path(path);
    let result = File::create(&temp)
        .and_then(|file| write_buffered(file, write))
        .and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        // Nothing to clean when it could not even be created.
        std::fs::remove_file(&temp).ok();
    }
    result
}

/// Write the `kind` output file `path` with `write` (see [`replace_file`]). Errors (no
/// write permission, full disk) are logged instead of panicking: the other outputs are
/// still generated.
fn write_output(
    path: &std::path::Path,
    kind: &str,
    write: impl FnOnce(&mut dyn Write) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let result = replace_file(path, |writer| write(writer));
    match &result {
        Ok(()) => log::info!("Generated {kind}: {:?}", path),
        Err(e) => log::error!("Cannot write {kind} {:?}: {}", path, e),
    }
    result
}

/// The built-in CSS output; the LESS one is identical.
fn generate_css(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

fn generate_html(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
    let name = sprite.name();
    let entries = entries(config, sprite);
    writeln!(f, "<html>
    <head><title>Spriters Test Html</title>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"{}.css\">
    <style type=\"text/css\">tr div:hover{{border:1px solid #ccc;}} tr div{{border:1px solid white;}}</style>
    </head><body>
    <h1>CSS Classes ({})</h1>
    <table>
    <thead><tr>
        <th>CSS Class</th>
        <th>Result</th>
    </tr></thead><tbody>", name, entries.len())?;
    for entry in &entries {
        // Same class names as the generated stylesheet (namespace, separator…). An entry
        // with a pseudo-class shows when its state applies (e.g. on hover).
        writeln!(
            f,
            "<tr><td>{selector}</td><td><div class=\"{class}\"></div></td></tr>",
            selector = entry.selector(),
            class = entry.class,
        )?;
    }
    write!(f, "</tbody></table>
    <footer>Generated using <a href=\"https://codeberg.org/slundi/spriters\">Spriters v{}</a></footer>
    </body></html>", SPRITERS_VERSION)?;
    Ok(())
}

fn generate_scss(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

fn generate_css_vars(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

fn generate_coco2d(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

fn generate_json(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

fn generate_caat(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

/// Write the sprite image of every ratio. Returns whether all of them were written.
fn generate_sprite_image(config: &Config, sprite: &Sprite) -> bool {
    let (width, height) = sprite.dimensions();
    if width == 0 || height == 0 {
        // Only an empty sprite, reported as a failure by the manager.
        log::warn!("Sprite has zero dimensions, skipping image generation");
        return true;
    }

    // Pixels are at the highest ratio: lower ratios are scaled down from them.
    let max_ratio = sprite.ratios.last().copied().unwrap_or(1.0);
    let filenames = sprite_filenames(config, sprite);
    let mut written = 0;
    for (&ratio, filename) in sprite.ratios.iter().zip(&filenames) {
        let canvas = render_sprite(config, sprite, ratio, max_ratio);
        if save_sprite_image(config, canvas, &config.image_dir().join(filename)) {
            written += 1;
        }
    }
    if written > 0 {
        // Every current file is kept, written or not: one that failed to be replaced still
        // holds its previous, complete content.
        remove_stale_hashed_sprites(config, sprite, &filenames);
    }
    written == sprite.ratios.len()
}

/// Length in pixels at `ratio` of `length` 1× units.
fn scaled(length: u32, ratio: f32) -> u32 {
    (length as f32 * ratio).ceil() as u32
}

/// Compose the sprite image at `ratio`, from images whose pixels are at `max_ratio`.
///
/// Positions are rounded and sizes floored, so an image never spills into its neighbour:
/// `round(x·r) + floor(w·r) <= round((x + w)·r)`.
fn render_sprite(config: &Config, sprite: &Sprite, ratio: f32, max_ratio: f32) -> RgbaImage {
    let (width, height) = sprite.dimensions();
    let mut canvas = RgbaImage::new(scaled(width, ratio), scaled(height, ratio));
    let scale = ratio / max_ratio;

    for img in sprite.images.iter() {
        // Images are stored as RGBA8 by discovery: borrowed, not copied.
        let mut rgba_img: std::borrow::Cow<RgbaImage> = match img.image.as_rgba8() {
            Some(buffer) => std::borrow::Cow::Borrowed(buffer),
            None => std::borrow::Cow::Owned(img.image.to_rgba8()),
        };
        if scale != 1.0 {
            let target_w = ((rgba_img.width() as f32 * scale).floor() as u32).max(1);
            let target_h = ((rgba_img.height() as f32 * scale).floor() as u32).max(1);
            rgba_img = std::borrow::Cow::Owned(image::imageops::resize(
                &*rgba_img,
                target_w,
                target_h,
                config.resize_algorithm,
            ));
        }
        let (origin_x, origin_y) = img.pixel_origin(&sprite.padding);
        let x = (origin_x as f32 * ratio).round() as u32;
        let y = (origin_y as f32 * ratio).round() as u32;
        if let Err(e) = canvas.copy_from(&*rgba_img, x, y) {
            log::error!("Failed to copy image {} to sprite: {}", img.path, e);
        }
    }
    canvas
}

/// Save a sprite image, logging the outcome. Returns whether it was written.
fn save_sprite_image(config: &Config, canvas: RgbaImage, output_path: &std::path::Path) -> bool {
    let (width, height) = canvas.dimensions();
    // png8 only applies to PNG; WebP and AVIF handle compression natively.
    let png8 = config.png8 && config.sprite_format == SpriteFormat::Png;
    let save_result = replace_file(output_path, |writer| {
        if png8 {
            encode_png8(&canvas, writer)
        } else {
            encode_image(&canvas, config.sprite_format.image_format(), writer)
        }
    });

    match save_result {
        Err(e) => {
            log::error!("Failed to save sprite image to {:?}: {}", output_path, e);
            false
        }
        Ok(()) => {
            let mode = if png8 { "8-bit palette" } else { "RGBA" };
            log::info!(
                "Generated sprite: {:?} ({}x{}, {})",
                output_path,
                width,
                height,
                mode
            );
            true
        }
    }
}

/// Encode `canvas` in `format` into `writer`.
fn encode_image(
    canvas: &RgbaImage,
    format: image::ImageFormat,
    writer: &mut (impl Write + std::io::Seek),
) -> std::io::Result<()> {
    canvas.write_to(writer, format).map_err(|e| match e {
        image::ImageError::IoError(e) => e,
        other => std::io::Error::other(other),
    })
}

/// Encode `canvas` as an 8-bit palette PNG (`--png8`) into `writer`: up to 256 colours,
/// transparency kept through the `tRNS` chunk.
fn encode_png8(canvas: &RgbaImage, writer: &mut impl Write) -> std::io::Result<()> {
    let (palette, indices) = quantize(canvas);
    let mut encoder = png::Encoder::new(writer, canvas.width(), canvas.height());
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(
        palette
            .iter()
            .flat_map(|color| [color[0], color[1], color[2]])
            .collect::<Vec<u8>>(),
    );
    // One alpha per palette entry; trailing opaque entries may be left out.
    let mut alpha: Vec<u8> = palette.iter().map(|color| color[3]).collect();
    while alpha.last() == Some(&255) {
        alpha.pop();
    }
    if !alpha.is_empty() {
        encoder.set_trns(alpha);
    }
    let png_error = |e: png::EncodingError| match e {
        png::EncodingError::IoError(e) => e,
        other => std::io::Error::other(other),
    };
    let mut writer = encoder.write_header().map_err(png_error)?;
    writer.write_image_data(&indices).map_err(png_error)?;
    // Writes the end chunk and flushes; dropping the writer would ignore their errors.
    writer.finish().map_err(png_error)
}

/// Reduce `canvas` to a palette of at most 256 RGBA colours and one palette index per pixel.
///
/// Up to 256 distinct colours are kept exactly. Beyond, the visible pixels are quantised
/// with [`median_cut`] into 255 colours, entry 0 being reserved for full transparency so that
/// it stays exact. Fully transparent pixels always share a single entry, whatever their RGB.
fn quantize(canvas: &RgbaImage) -> (Vec<[u8; 4]>, Vec<u8>) {
    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    let normalized = |pixel: &image::Rgba<u8>| if pixel.0[3] == 0 { CLEAR } else { pixel.0 };

    let mut exact: std::collections::HashMap<[u8; 4], u8> = std::collections::HashMap::new();
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let fits = canvas.pixels().all(|pixel| {
        let color = normalized(pixel);
        if exact.contains_key(&color) {
            return true;
        }
        if palette.len() == 256 {
            return false;
        }
        exact.insert(color, palette.len() as u8);
        palette.push(color);
        true
    });
    if fits {
        let indices = canvas
            .pixels()
            .map(|pixel| exact[&normalized(pixel)])
            .collect();
        return (palette, indices);
    }

    let mut counts: std::collections::HashMap<[u8; 4], u32> = std::collections::HashMap::new();
    for pixel in canvas.pixels().filter(|pixel| pixel.0[3] != 0) {
        *counts.entry(pixel.0).or_default() += 1;
    }
    let (visible_palette, visible_index) = median_cut(counts.into_iter().collect(), 255);
    let palette = std::iter::once(CLEAR).chain(visible_palette).collect();
    let indices = canvas
        .pixels()
        .map(|pixel| {
            if pixel.0[3] == 0 {
                0
            } else {
                visible_index[&pixel.0] + 1
            }
        })
        .collect();
    (palette, indices)
}

/// Median-cut quantisation of distinct RGBA `colors` (with their pixel count) into at most
/// `max_colors` entries: the box with the widest channel is split at its weighted median until
/// there are enough boxes, each box then giving the weighted average of its colours.
///
/// Returns the palette and the palette index of every input colour. Deterministic: the result
/// does not depend on the order of `colors`.
fn median_cut(
    mut colors: Vec<([u8; 4], u32)>,
    max_colors: usize,
) -> (Vec<[u8; 4]>, std::collections::HashMap<[u8; 4], u8>) {
    /// Channel with the widest value range in `colors`, and that range.
    fn widest_channel(colors: &[([u8; 4], u32)]) -> (usize, u8) {
        (0..4)
            .map(|channel| {
                let values = colors.iter().map(|(color, _)| color[channel]);
                let range = values.clone().max().unwrap_or(0) - values.min().unwrap_or(0);
                (channel, range)
            })
            .max_by_key(|&(channel, range)| (range, std::cmp::Reverse(channel)))
            .unwrap_or((0, 0))
    }

    colors.sort_unstable();
    let mut boxes = vec![colors];
    while boxes.len() < max_colors {
        let Some((index, channel)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, colors)| colors.len() > 1)
            .map(|(index, colors)| (index, widest_channel(colors)))
            .max_by_key(|&(index, (_, range))| (range, std::cmp::Reverse(index)))
            .map(|(index, (channel, _))| (index, channel))
        else {
            break;
        };
        let mut colors = boxes.remove(index);
        colors.sort_unstable_by_key(|&(color, _)| (color[channel], color));
        // Weighted median, keeping at least one colour on each side.
        let total: u64 = colors.iter().map(|&(_, count)| count as u64).sum();
        let mut seen = 0u64;
        let split = colors
            .iter()
            .position(|&(_, count)| {
                seen += count as u64;
                seen * 2 >= total
            })
            .map_or(1, |position| position + 1)
            .clamp(1, colors.len() - 1);
        let upper = colors.split_off(split);
        boxes.push(colors);
        boxes.push(upper);
    }

    let mut index_of = std::collections::HashMap::new();
    let palette = boxes
        .iter()
        .enumerate()
        .map(|(index, colors)| {
            let total: u64 = colors.iter().map(|&(_, count)| count as u64).sum();
            let mut average = [0u8; 4];
            for (channel, value) in average.iter_mut().enumerate() {
                let sum: u64 = colors
                    .iter()
                    .map(|&(color, count)| color[channel] as u64 * count as u64)
                    .sum();
                *value = ((sum + total / 2) / total) as u8;
            }
            for &(color, _) in colors {
                index_of.insert(color, index as u8);
            }
            average
        })
        .collect();
    (palette, index_of)
}

/// A sprite image file name `name[_<8 hex>][@<ratio>x].<ext>` (see [`sprite_filename_at`]).
struct SpriteImageName<'a> {
    hashed: bool,
    ratio_suffix: bool,
    ext: &'a str,
}

/// Parse `file_name` as an image of the sprite `name`, `None` if it is not one.
fn parse_sprite_image_name<'a>(file_name: &'a str, name: &str) -> Option<SpriteImageName<'a>> {
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

/// Remove sprite images left by previous runs (older cache busters or ratios), so they do
/// not pile up in the output directory. Only cache-busted (`_<8 hex digits>`) or ratio
/// (`@<ratio>x`) images with the current extension are removed; `current` files are kept.
fn remove_stale_hashed_sprites(config: &Config, sprite: &Sprite, current: &[String]) {
    let Ok(entries) = std::fs::read_dir(config.image_dir()) else {
        return;
    };
    let name = sprite.name();
    let ext = config.sprite_format.extension();

    for entry in entries.filter_map(|entry| entry.ok()) {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let is_stale = !current.contains(&file_name)
            && parse_sprite_image_name(&file_name, &name)
                .is_some_and(|image| image.ext == ext && (image.hashed || image.ratio_suffix));
        if is_stale {
            match std::fs::remove_file(entry.path()) {
                Ok(()) => log::info!("Removed stale sprite: {:?}", entry.path()),
                Err(e) => log::warn!("Cannot remove stale sprite {:?}: {}", entry.path(), e),
            }
        }
    }
}

/// The sprite image of `name` in `output_dir`, for metadata that does not record it: among
/// its default images (plain or cache-busted, see [`sprite_filename`], in any format, not an
/// `@<ratio>x` one), the most recently written. The others may be left from a previous
/// format or cache buster.
pub(crate) fn find_sprite_image(output_dir: &std::path::Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(output_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            parse_sprite_image_name(&file_name, name).is_some_and(|image| !image.ratio_suffix)
        })
        .filter_map(|entry| {
            let metadata = entry
                .metadata()
                .ok()
                .filter(|metadata| metadata.is_file())?;
            // Ties (coarse timestamps) are broken by name, so the result is stable.
            Some((metadata.modified().ok()?, entry.path()))
        })
        .max()
        .map(|(_, path)| path)
}

/// Name of the metadata file for the sprite `name`.
pub(crate) fn meta_file_name(name: &str) -> String {
    format!(".{name}.spriters")
}

/// Get the path to the metadata file for a sprite
fn get_meta_path(config: &Config, sprite: &Sprite) -> PathBuf {
    let name = sprite.name();
    config.image_dir().join(meta_file_name(&name))
}

/// Compute a hash of all inputs that affect sprite generation
fn compute_sprite_hash(config: &Config, sprite: &Sprite) -> u64 {
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
fn compute_outputs_hash(config: &Config, sprite: &Sprite) -> u64 {
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

/// Writes the content of a text output; every write error is returned.
type TextWriter = fn(&Config, &Sprite, &mut dyn Write) -> std::io::Result<()>;

/// Environment of the custom templates (`--css-template`…): Jinja syntax, no HTML escaping
/// (the outputs are stylesheets), and the final newline of the template is kept.
fn template_environment() -> minijinja::Environment<'static> {
    let mut environment = minijinja::Environment::new();
    environment.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    if let Ok(syntax) = minijinja::syntax::SyntaxConfig::builder()
        .keep_trailing_newline(true)
        .build()
    {
        environment.set_syntax(syntax);
    }
    environment
}

/// Read and compile the template `path`, so that a missing file or a syntax error is
/// reported before anything is generated. Returns the error message.
pub(crate) fn check_template(path: &std::path::Path) -> Result<(), String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    template_environment()
        .template_from_str(&source)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Variables given to a custom template, see the README.
fn template_context(config: &Config, sprite: &Sprite) -> minijinja::Value {
    use minijinja::{Value, context};

    let (width, height) = sprite.dimensions();
    let entries = entries(config, sprite);
    let count = entries.len();
    let images: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let img = entry.image;
            let filename = std::path::Path::new(entry.path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            context! {
                label => entry.class.clone(),
                pseudo => entry.pseudo.clone(),
                selector => entry.selector(),
                name => entry.label.clone(),
                filename,
                path => entry.path,
                x => img.x,
                y => img.y,
                width => img.computed_width,
                height => img.computed_height,
                last => index + 1 == count,
            }
        })
        .collect();
    let ratios: Vec<Value> = sprite
        .ratios
        .iter()
        .skip(1)
        .map(|&ratio| {
            let sprite_filename = sprite_filename_at(config, sprite, ratio);
            context! {
                ratio => ratio.to_string(),
                fraction => crate::util::nearest_fration(ratio),
                sprite_path => sprite_file_url(config, sprite, sprite_filename.clone()),
                sprite_filename,
            }
        })
        .collect();
    context! {
        name => sprite.name(),
        sprite_path => get_sprite_url(config, sprite),
        sprite_filename => sprite_filename(config, sprite),
        width,
        height,
        hash => cache_buster(config, sprite),
        images,
        ratios,
    }
}

/// Render the custom template `path` for `sprite` into `f`.
fn render_template(
    path: &std::path::Path,
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let failed = |error: String| format!("template {}: {error}", path.display());
    let source = std::fs::read_to_string(path)
        .map_err(|e| std::io::Error::new(e.kind(), failed(e.to_string())))?;
    let rendered = template_environment()
        .template_from_str(&source)
        .and_then(|template| template.render(template_context(config, sprite)))
        .map_err(|e| std::io::Error::other(failed(e.to_string())))?;
    f.write_all(rendered.as_bytes())
}

/// Write with the custom `template` when there is one, with `builtin` otherwise.
fn templated(
    template: &Option<PathBuf>,
    builtin: TextWriter,
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    match template {
        Some(path) => render_template(path, config, sprite, f),
        None => builtin(config, sprite, f),
    }
}

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
            templated(&config.css_template, generate_css, config, sprite, f)
        }),
        // The built-in LESS output is the CSS one.
        (config.less, "less", "LESS", |config, sprite, f| {
            templated(&config.less_template, generate_css, config, sprite, f)
        }),
        (config.scss, "scss", "SCSS", |config, sprite, f| {
            templated(&config.scss_template, generate_scss, config, sprite, f)
        }),
        (config.json, "json", "JSON", generate_json),
        (config.html, "html", "HTML", generate_html),
        (config.cocos2d, "plist", "Cocos2d plist", generate_coco2d),
        (config.caat, "caat.json", "CAAT", generate_caat),
        (
            config.css_vars,
            "vars.css",
            "CSS vars",
            |config, sprite, f| {
                templated(
                    &config.css_vars_template,
                    generate_css_vars,
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
            generate_tailwind,
        ),
        (config.dts, "d.ts", "TypeScript definitions", generate_dts),
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

/// Sprite image extensions, see [`SpriteFormat::extension`].
const SPRITE_EXTENSIONS: [&str; 3] = ["png", "webp", "avif"];

/// Every suffix (after `<sprite name>.`) of a file spriters may write, whatever the options:
/// the sprite image and all text outputs (see [`text_output_names`]).
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
/// temporary file of one of them (see [`temp_path`]).
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

/// Data stored in the metadata file by the previous run.
#[derive(Debug, Default)]
pub(crate) struct Metadata {
    pub image_hash: Option<u64>,
    pub outputs_hash: Option<u64>,
    /// File name of the sprite image written by the previous run (see [`sprite_filename`]).
    pub image_file: Option<String>,
}

fn read_metadata(config: &Config, sprite: &Sprite) -> Metadata {
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

fn staleness_from(config: &Config, sprite: &Sprite, stored: &Metadata) -> Staleness {
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
fn save_metadata(
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

fn generate_tailwind(config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

fn generate_dts(_config: &Config, sprite: &Sprite, f: &mut dyn Write) -> std::io::Result<()> {
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

/// What `--dry-run` prints for `sprite`: its classes and the files that would be written.
fn dry_run_report(config: &Config, sprite: &Sprite) -> String {
    let name = sprite.name();
    let (w, h) = sprite.dimensions();
    let total_entries: usize = sprite.images.iter().map(|i| 1 + i.aliases.len()).sum();

    let mut lines = vec![format!(
        "Sprite: {} ({}×{} px, {} entries)",
        name, w, h, total_entries
    )];
    for entry in entries(config, sprite) {
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
    let stale = staleness_from(config, sprite, &stored);
    if !stale.any() {
        log::info!("Sprite is up to date, skipping generation");
        return true;
    }

    let image_written = !stale.image || generate_sprite_image(config, sprite);
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
    fn test_write_output_returns_writer_errors() {
        let dir =
            std::env::temp_dir().join(format!("spriters_write_output_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("icons.css");

        assert!(write_output(&path, "CSS", |f| f.write_all(b".a {}")).is_ok());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ".a {}");
        assert!(write_output(&path, "CSS", |_| Err(std::io::Error::other("boom"))).is_err());
        // Cannot be created: the path is a directory.
        assert!(write_output(&dir, "CSS", |f| f.write_all(b".a {}")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `/dev/full` accepts the file creation, then fails every write with "no space left".
    #[cfg(target_os = "linux")]
    #[test]
    fn test_full_disk_is_reported_on_flush() {
        let full = || File::create("/dev/full").unwrap();
        // A few buffered bytes only fail when the `BufWriter` is flushed.
        assert!(write_buffered(full(), |f| f.write_all(b".a {}")).is_err());

        let canvas = RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        for format in [image::ImageFormat::Png, image::ImageFormat::WebP] {
            let result = write_buffered(full(), |f| encode_image(&canvas, format, f));
            assert!(result.is_err(), "{format:?}");
        }
        assert!(write_buffered(full(), |f| encode_png8(&canvas, f)).is_err());
    }

    /// Names of the files in `dir`, sorted.
    fn dir_entries(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn test_replace_file_keeps_previous_content_on_error() {
        // Review 6.3: `File::create` truncated the file first, so a failed or interrupted
        // write left a partial file behind.
        let dir = std::env::temp_dir().join(format!("spriters_replace_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("icons.css");
        std::fs::write(&path, ".old {}").unwrap();

        let failed = replace_file(&path, |f| {
            f.write_all(b".new")?;
            f.flush()?; // the partial content reaches the disk
            Err(std::io::Error::other("interrupted"))
        });
        assert!(failed.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ".old {}");
        assert_eq!(
            dir_entries(&dir),
            ["icons.css"],
            "temporary file left behind"
        );

        replace_file(&path, |f| f.write_all(b".new {}")).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ".new {}");
        assert_eq!(dir_entries(&dir), ["icons.css"]);

        // A new file appears complete or not at all.
        let created = dir.join("icons.json");
        assert!(replace_file(&created, |_| Err(std::io::Error::other("boom"))).is_err());
        assert!(!created.exists());
        // The content is complete while it is written: never visible under its name.
        replace_file(&created, |f| {
            assert!(!created.exists());
            f.write_all(b"{}")
        })
        .unwrap();
        assert_eq!(dir_entries(&dir), ["icons.css", "icons.json"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_failed_image_keeps_previous_image_and_metadata() {
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
            staleness_from(
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

    #[test]
    fn test_temp_path_round_trip() {
        let temp = temp_path(std::path::Path::new("out/icons.css"));
        assert_eq!(temp.parent(), Some(std::path::Path::new("out")));
        let name = temp.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, format!(".icons.css.{}.tmp", std::process::id()));
        assert_eq!(temp_target(name), Some("icons.css"));
        assert_eq!(
            temp_target("..icons.spriters.12.tmp"),
            Some(".icons.spriters")
        );

        for name in [
            "icons.css",
            ".icons.css",
            "icons.css.12.tmp",
            ".icons.css.tmp",
            ".icons.css.12a.tmp",
            "..12.tmp",
            ".icons.css..tmp",
        ] {
            assert_eq!(temp_target(name), None, "{name}");
        }
    }

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
        for file in text_output_names(&every_output, "america") {
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

    #[test]
    fn test_quantize_keeps_few_colours_exactly() {
        let mut canvas = RgbaImage::new(4, 2);
        canvas.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        canvas.put_pixel(1, 0, image::Rgba([0, 0, 255, 128]));
        // Fully transparent pixels share one entry whatever their RGB.
        canvas.put_pixel(2, 0, image::Rgba([9, 9, 9, 0]));
        let (palette, indices) = quantize(&canvas);

        assert_eq!(palette.len(), 3);
        for (pixel, index) in canvas.pixels().zip(&indices) {
            let expected = if pixel.0[3] == 0 {
                [0, 0, 0, 0]
            } else {
                pixel.0
            };
            assert_eq!(palette[*index as usize], expected);
        }
    }

    #[test]
    fn test_quantize_many_colours_keeps_transparency_exact() {
        // 32×32 gradient (1024 colours) with a transparent first row.
        let canvas = RgbaImage::from_fn(32, 32, |x, y| {
            if y == 0 {
                image::Rgba([200, 10, 10, 0])
            } else {
                image::Rgba([(x * 8) as u8, (y * 8) as u8, 128, 255])
            }
        });
        let (palette, indices) = quantize(&canvas);

        assert!(palette.len() <= 256);
        assert_eq!(
            palette[0],
            [0, 0, 0, 0],
            "entry 0 is reserved for transparency"
        );
        for (pixel, index) in canvas.pixels().zip(&indices) {
            let color = palette[*index as usize];
            if pixel.0[3] == 0 {
                assert_eq!(*index, 0);
            } else {
                assert_ne!(*index, 0, "a visible pixel must not become transparent");
                for channel in 0..3 {
                    let diff = (color[channel] as i32 - pixel.0[channel] as i32).abs();
                    assert!(diff <= 24, "{:?} -> {:?}", pixel.0, color);
                }
            }
        }
    }

    #[test]
    fn test_median_cut_is_deterministic_and_bounded() {
        let colors: Vec<([u8; 4], u32)> = (0..600u32)
            .map(|value| {
                (
                    [(value % 256) as u8, (value / 3) as u8, 7, 255],
                    value % 5 + 1,
                )
            })
            .collect();
        let mut reversed = colors.clone();
        reversed.reverse();

        let (palette, index_of) = median_cut(colors.clone(), 255);
        assert_eq!(palette.len(), 255);
        assert_eq!(
            median_cut(reversed, 255),
            (palette.clone(), index_of.clone())
        );
        for (color, _) in &colors {
            assert!((index_of[color] as usize) < palette.len());
        }
        // Fewer colours than entries: each keeps its own entry, exactly.
        let few = vec![([1, 2, 3, 255], 4), ([200, 100, 50, 128], 1)];
        let (palette, index_of) = median_cut(few, 255);
        assert_eq!(palette.len(), 2);
        assert_eq!(
            palette[index_of[&[200, 100, 50, 128]] as usize],
            [200, 100, 50, 128]
        );
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

    #[test]
    fn test_css_pseudo_classes_constant() {
        // Verify known pseudo classes are present
        assert!(crate::CSS_PSEUDO_CLASSES.contains(&"hover"));
        assert!(crate::CSS_PSEUDO_CLASSES.contains(&"active"));
        assert!(crate::CSS_PSEUDO_CLASSES.contains(&"focus"));
        assert_eq!(crate::CSS_PSEUDO_CLASSES.len(), 10);
    }
}
