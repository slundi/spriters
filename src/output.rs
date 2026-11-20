use image::{GenericImage, RgbaImage};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use crate::{Config, JsonFormat, Sprite, SpriteFormat};

const SPRITERS_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
const CSS_PSEUDO_CLASSES: [&str; 10] = [
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
        None => filename,
    }
}

/// Build a CSS class name based on config settings (namespace, sprite_namespace, separator).
/// Default pattern: `sprite-{sprite_name}-{image_label}`
fn build_class_name(config: &Config, sprite_name: &str, image_label: &str) -> String {
    let sep = &config.separator;

    // Determine namespace (default: "sprite")
    let namespace = config.namespace.as_deref().unwrap_or("sprite");

    // Determine sprite namespace (default: sprite name, supports %(sprite)s placeholder)
    let sprite_ns = match &config.sprite_namespace {
        Some(sn) => sn.replace("%(sprite)s", sprite_name),
        None => sprite_name.to_string(),
    };

    // Build the class name
    let mut class_name = String::new();
    if !namespace.is_empty() {
        class_name.push_str(namespace);
        if !sprite_ns.is_empty() || !image_label.is_empty() {
            class_name.push_str(sep);
        }
    }
    if !sprite_ns.is_empty() {
        class_name.push_str(&sprite_ns);
        if !image_label.is_empty() {
            class_name.push_str(sep);
        }
    }
    class_name.push_str(image_label);

    class_name
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
    f: &mut impl Write,
    config: &Config,
    sprite: &Sprite,
    selectors: &str,
) {
    for &ratio in sprite.ratios.iter().skip(1) {
        let url = sprite_file_url(config, sprite, sprite_filename_at(config, sprite, ratio));
        let _ = writeln!(
            f,
            "\n@media screen and (-webkit-min-device-pixel-ratio: {r}), screen and (min--moz-device-pixel-ratio: {r}), screen and (-o-min-device-pixel-ratio: {frac}), screen and (min-device-pixel-ratio: {r}), screen and (min-resolution: {r}dppx) {{",
            r = ratio,
            frac = crate::util::nearest_fration(ratio),
        );
        let _ = writeln!(
            f,
            "  .{selectors} {{\n    background-image: url('{url}');\n  }}\n}}"
        );
    }
}

/// Create an output file, logging the error instead of panicking (e.g. no write permission
/// or a full disk): the other outputs are still generated.
fn create_output(path: &std::path::Path) -> Option<BufWriter<File>> {
    File::create(path)
        .map(BufWriter::new)
        .map_err(|e| log::error!("Cannot create {:?}: {}", path, e))
        .ok()
}

/// Derive a CSS-safe label from an arbitrary file path (same logic as `ImageAndPosition::label()`).
fn label_from_path(path: &str) -> String {
    slug::slugify(
        std::path::Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
    )
}

fn implode_image_label(config: &Config, sprite: &Sprite) -> String {
    let sprite_name = sprite.name();
    let mut labels: Vec<String> = Vec::with_capacity(sprite.images.len());
    for img in sprite.images.iter() {
        labels.push(build_class_name(config, &sprite_name, &img.label()));
        for alias_path in &img.aliases {
            labels.push(build_class_name(
                config,
                &sprite_name,
                &label_from_path(alias_path),
            ));
        }
    }
    labels.join(",")
}

fn generate_css(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.css", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
    let sprite_url = get_sprite_url(config, sprite);
    let selectors = implode_image_label(config, sprite).replace(",", ", .");

    // Write the shared class for all images
    let _ = writeln!(
        &mut f,
        ".{list} {{\n  background-image: url('{url}');\n  background-repeat: no-repeat;{size}\n}}",
        list = selectors,
        url = sprite_url,
        size = background_size(sprite),
    );

    // Write individual image classes (primary + aliases share the same rule).
    for img in sprite.images.iter() {
        let x = if img.x == 0 {
            String::from("0")
        } else {
            format!("-{}px", img.x)
        };
        let y = if img.y == 0 {
            String::from("0")
        } else {
            format!("-{}px", img.y)
        };
        let selector = std::iter::once(img.label())
            .chain(img.aliases.iter().map(|p| label_from_path(p)))
            .map(|lbl| format!(".{}", build_class_name(config, &name, &lbl)))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            &mut f,
            "\n{selector} {{\n  background-position: {x} {y};\n  width: {w}px;\n  height: {h}px;\n}}",
            selector = selector,
            x = x,
            y = y,
            w = img.computed_width,
            h = img.computed_height,
        );
    }

    write_ratio_media_queries(&mut f, config, sprite, &selectors);

    log::info!("Generated CSS: {:?}", output_path);
}

fn generate_html(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.html", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
    let total = sprite
        .images
        .iter()
        .map(|img| 1 + img.aliases.len())
        .sum::<usize>();
    let _ = writeln!(&mut f, "<html>
    <head><title>Spriters Test Html</title>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"{}.css\">
    <style type=\"text/css\">tr div:hover{{border:1px solid #ccc;}} tr div{{border:1px solid white;}}</style>
    </head><body>
    <h1>CSS Classes ({})</h1>
    <table>
    <thead><tr>
        <th>CSS Class</th>
        <th>Result</th>
    </tr></thead><tbody>", name, total);
    for image in sprite.images.iter() {
        for label in
            std::iter::once(image.label()).chain(image.aliases.iter().map(|p| label_from_path(p)))
        {
            // Same class names as the generated stylesheet (namespace, separator…).
            let _ = writeln!(
                &mut f,
                "<tr><td>.{class}</td><td><div class=\"{class}\"></div></td></tr>",
                class = build_class_name(config, &name, &label)
            );
        }
    }
    let _ = write!(&mut f, "</tbody></table>
    <footer>Generated using <a href=\"https://codeberg.org/slundi/spriters\">Spriters v{}</a></footer>
    </body></html>", SPRITERS_VERSION);
    log::info!("Generated HTML: {:?}", output_path);
}
const _LESS_TEMPLATE: &str = "<% for image in &images { %>.<%= image.label %><%= image.pseudo %><%- if !image.last %>,\n<% } %><% } %>{
    background-image:url('<%= sprite_path %'); background-repeat:no-repeat;
    -webkit-background-size: {{ width }}px {{ height }}px;
    -moz-background-size: {{ width }}px {{ height }}px;
    background-size: {{ width }}px {{ height }}px;
    {% for r, ratio in ratios.items() %}
    @media screen and (-webkit-min-device-pixel-ratio: {{ ratio.ratio }}), screen and (min--moz-device-pixel-ratio: {{ ratio.ratio }}),screen and (-o-min-device-pixel-ratio: {{ ratio.fraction }}),screen and (min-device-pixel-ratio: {{ ratio.ratio }}),screen and (min-resolution: {{ ratio.ratio }}dppx){
        background-image:url('<%= ratio.sprite_path %');
    }
    <% } %>
}
<% for image in &images { %>
.<%= image.label %><%= image.pseudo %>{
    background-position:{{ image.x ~ ('px' if image.x) }} {{ image.y ~ ('px' if image.y) }};
    width:<%= image.width %>px;
    height:<%= image.height %>px;
}
<% } %>";

fn generate_less(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.less", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
    let sprite_url = get_sprite_url(config, sprite);
    let selectors = implode_image_label(config, sprite).replace(",", ", .");
    let _ = writeln!(
        f,
        ".{list} {{\n  background-image: url('{url}');\n  background-repeat: no-repeat;{size}\n}}",
        list = selectors,
        url = sprite_url,
        size = background_size(sprite),
    );
    for img in sprite.images.iter() {
        let x = if img.x == 0 {
            String::from("0")
        } else {
            format!("-{}px", img.x)
        };
        let y = if img.y == 0 {
            String::from("0")
        } else {
            format!("-{}px", img.y)
        };
        let selector = std::iter::once(img.label())
            .chain(img.aliases.iter().map(|p| label_from_path(p)))
            .map(|lbl| format!(".{}", build_class_name(config, &name, &lbl)))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            f,
            "\n{selector} {{\n  background-position: {x} {y};\n  width: {w}px;\n  height: {h}px;\n}}",
            selector = selector,
            x = x,
            y = y,
            w = img.computed_width,
            h = img.computed_height,
        );
    }
    write_ratio_media_queries(&mut f, config, sprite, &selectors);
    log::info!("Generated LESS: {:?}", output_path);
}

fn generate_scss(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.scss", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
    let (width, height) = sprite.dimensions();
    let sprite_url = get_sprite_url(config, sprite);

    // Generate a mixin for the sprite base
    let _ = writeln!(
        f,
        "@mixin {name}-sprite {{\n  background-image: url('{url}');\n  background-repeat: no-repeat;\n  background-size: {w}px {h}px;\n}}",
        name = name,
        url = sprite_url,
        w = width,
        h = height,
    );

    // Generate the shared class
    let _ = writeln!(
        f,
        "\n.{list} {{\n  @include {name}-sprite;\n}}",
        list = implode_image_label(config, sprite).replace(",", ",."),
        name = name,
    );

    // Generate individual image classes (primary + aliases share the same rule).
    for img in sprite.images.iter() {
        let selector = std::iter::once(img.label())
            .chain(img.aliases.iter().map(|p| label_from_path(p)))
            .map(|lbl| format!(".{}", build_class_name(config, &name, &lbl)))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            f,
            "\n{selector} {{\n  background-position: -{x}px -{y}px;\n  width: {w}px;\n  height: {h}px;\n}}",
            selector = selector,
            x = img.x,
            y = img.y,
            w = img.computed_width,
            h = img.computed_height,
        );
    }
    // The mixin already sets `background-size`.
    let selectors = implode_image_label(config, sprite).replace(",", ", .");
    write_ratio_media_queries(&mut f, config, sprite, &selectors);
}

fn generate_css_vars(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.vars.css", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
    let sprite_url = get_sprite_url(config, sprite);

    let _ = writeln!(f, ":root {{");
    let _ = writeln!(
        f,
        "  --{name}-image: url('{url}');",
        name = name,
        url = sprite_url
    );

    for img in sprite.images.iter() {
        let x_val = if img.x == 0 {
            "0".to_owned()
        } else {
            format!("-{}px", img.x)
        };
        let y_val = if img.y == 0 {
            "0".to_owned()
        } else {
            format!("-{}px", img.y)
        };

        for label in
            std::iter::once(img.label()).chain(img.aliases.iter().map(|p| label_from_path(p)))
        {
            let prop = build_class_name(config, &name, &label);
            let _ = writeln!(f, "  --{p}-x: {x};", p = prop, x = x_val);
            let _ = writeln!(f, "  --{p}-y: {y};", p = prop, y = y_val);
            let _ = writeln!(f, "  --{p}-w: {w}px;", p = prop, w = img.computed_width);
            let _ = writeln!(f, "  --{p}-h: {h}px;", p = prop, h = img.computed_height);
        }
    }

    let _ = writeln!(f, "}}");
    log::info!("Generated CSS vars: {:?}", output_path);
}

fn generate_coco2d(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.plist", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
    let (width, height) = sprite.dimensions();
    let sprite_url = get_sprite_url(config, sprite);

    // Write plist header
    let _ = writeln!(f, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        f,
        r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">"#
    );
    let _ = writeln!(f, r#"<plist version="1.0">"#);
    let _ = writeln!(f, "<dict>");

    // Write frames (primary + aliases share the same region).
    let _ = writeln!(f, "    <key>frames</key>");
    let _ = writeln!(f, "    <dict>");

    for img in sprite.images.iter() {
        let labels: Vec<String> = std::iter::once(img.label())
            .chain(img.aliases.iter().map(|p| label_from_path(p)))
            .collect();
        for lbl in &labels {
            let _ = writeln!(f, "        <key>{}</key>", lbl);
            let _ = writeln!(f, "        <dict>");
            let _ = writeln!(f, "            <key>frame</key>");
            let _ = writeln!(
                f,
                "            <string>{{{{{},{}}},{{{},{}}}}}</string>",
                img.x, img.y, img.computed_width, img.computed_height
            );
            let _ = writeln!(f, "            <key>offset</key>");
            let _ = writeln!(f, "            <string>{{0,0}}</string>");
            let _ = writeln!(f, "            <key>rotated</key>");
            let _ = writeln!(f, "            <false/>");
            let _ = writeln!(f, "            <key>sourceColorRect</key>");
            let _ = writeln!(
                f,
                "            <string>{{{{0,0}},{{{},{}}}}}</string>",
                img.computed_width, img.computed_height
            );
            let _ = writeln!(f, "            <key>sourceSize</key>");
            let _ = writeln!(
                f,
                "            <string>{{{},{}}}</string>",
                img.computed_width, img.computed_height
            );
            let _ = writeln!(f, "        </dict>");
        }
    }

    let _ = writeln!(f, "    </dict>");

    // Write metadata
    let _ = writeln!(f, "    <key>metadata</key>");
    let _ = writeln!(f, "    <dict>");
    let _ = writeln!(f, "        <key>format</key>");
    let _ = writeln!(f, "        <integer>2</integer>");
    let _ = writeln!(f, "        <key>size</key>");
    let _ = writeln!(f, "        <string>{{{},{}}}</string>", width, height);
    let _ = writeln!(f, "        <key>textureFileName</key>");
    let _ = writeln!(f, "        <string>{}</string>", sprite_url);
    let _ = writeln!(f, "    </dict>");

    let _ = writeln!(f, "</dict>");
    let _ = writeln!(f, "</plist>");

    log::info!("Generated Cocos2d plist: {:?}", output_path);
}

fn generate_json(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.json", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
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
            let _ = writeln!(f, "{{\"frames\":[");
            for (index, (label, x, y, w, h)) in entries.iter().enumerate() {
                let _ = writeln!(
                    f,
                    "{{\"filename\": \"{label}\", \"frame\":{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}}}{coma}",
                    coma = if index + 1 < n { "," } else { "" },
                );
            }
            let _ = writeln!(
                f,
                "],\n\"meta\":{{\"width\": {w}, \"height\": {h}, \"sprite_filename\": \"{name}\", \"sprite_url\": \"{url}\"}}\n}}",
                w = width,
                h = height,
                url = sprite_url,
            );
        }
        JsonFormat::Hash => {
            let _ = writeln!(f, "{{");
            for (index, (label, x, y, w, h)) in entries.iter().enumerate() {
                let _ = writeln!(
                    f,
                    "\"{label}\": {{\"x\": {x}, \"y\": {y}, \"w\": {w}, \"h\": {h}}}{coma}",
                    coma = if index + 1 < n { "," } else { "" },
                );
            }
            let _ = writeln!(f, "}}");
        }
    }

    log::info!("Generated JSON: {:?}", output_path);
}

fn generate_caat(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.caat.json", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };
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

    let _ = writeln!(f, "{{");
    let _ = writeln!(f, "    \"sprites\": {{");
    for (index, (filename, x, y, w, h)) in entries.iter().enumerate() {
        let _ = writeln!(
            f,
            "        \"{fn}\": {{\"x\": {x}, \"y\": {y}, \"width\": {w}, \"height\": {h}}}{coma}",
            fn = filename, x = x, y = y, w = w, h = h,
            coma = if index + 1 < n { "," } else { "" },
        );
    }
    let _ = writeln!(f, "    }},");
    let _ = writeln!(f, "    \"meta\": {{");
    let _ = writeln!(f, "        \"version\": \"{}\",", SPRITERS_VERSION);
    let _ = writeln!(f, "        \"hash\": \"{:x}\",", hash);
    let _ = writeln!(f, "        \"sprite_filename\": \"{}\",", sprite_filename);
    let _ = writeln!(f, "        \"width\": {},", width);
    let _ = writeln!(f, "        \"height\": {}", height);
    let _ = writeln!(f, "    }}");
    let _ = writeln!(f, "}}");

    log::info!("Generated CAAT: {:?}", output_path);
}

fn generate_sprite_image(config: &Config, sprite: &Sprite) {
    let (width, height) = sprite.dimensions();
    if width == 0 || height == 0 {
        log::warn!("Sprite has zero dimensions, skipping image generation");
        return;
    }

    // Pixels are at the highest ratio: lower ratios are scaled down from them.
    let max_ratio = sprite.ratios.last().copied().unwrap_or(1.0);
    let mut written = Vec::with_capacity(sprite.ratios.len());
    for &ratio in &sprite.ratios {
        let canvas = render_sprite(config, sprite, ratio, max_ratio);
        let filename = sprite_filename_at(config, sprite, ratio);
        if save_sprite_image(config, canvas, &config.output.join(&filename)) {
            written.push(filename);
        }
    }
    if !written.is_empty() {
        remove_stale_hashed_sprites(config, sprite, &written);
    }
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
        let mut rgba_img = img.image.to_rgba8();
        if scale != 1.0 {
            let target_w = ((rgba_img.width() as f32 * scale).floor() as u32).max(1);
            let target_h = ((rgba_img.height() as f32 * scale).floor() as u32).max(1);
            rgba_img =
                image::imageops::resize(&rgba_img, target_w, target_h, config.resize_algorithm);
        }
        let (origin_x, origin_y) = img.pixel_origin(&sprite.padding);
        let x = (origin_x as f32 * ratio).round() as u32;
        let y = (origin_y as f32 * ratio).round() as u32;
        if let Err(e) = canvas.copy_from(&rgba_img, x, y) {
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
    let save_result = if png8 {
        save_png8(&canvas, output_path)
    } else {
        canvas.save(output_path).map_err(|e| e.to_string())
    };

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

/// Write `canvas` as an 8-bit palette PNG (`--png8`): up to 256 colours, transparency kept
/// through the `tRNS` chunk.
fn save_png8(canvas: &RgbaImage, path: &std::path::Path) -> Result<(), String> {
    let (palette, indices) = quantize(canvas);
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), canvas.width(), canvas.height());
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
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&indices).map_err(|e| e.to_string())
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
    let Ok(entries) = std::fs::read_dir(&config.output) else {
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

/// Name of the metadata file for the sprite `name`.
pub(crate) fn meta_file_name(name: &str) -> String {
    format!(".{name}.spriters")
}

/// Get the path to the metadata file for a sprite
fn get_meta_path(config: &Config, sprite: &Sprite) -> PathBuf {
    let name = sprite.name();
    config.output.join(meta_file_name(&name))
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

    // Hash each source image path, content, layout position, dimensions, and aliases
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
    (
        css_template,
        less_template,
        scss_template,
        css_vars_template,
    )
        .hash(&mut hasher);
    (no_css, html, less, scss, cocos2d, json, json_format).hash(&mut hasher);
    (caat, css_vars, tailwind, dts).hash(&mut hasher);
    hasher.finish()
}

/// File names of the text outputs enabled in `config`.
fn text_output_names(config: &Config, name: &str) -> Vec<String> {
    [
        (!config.no_css, "css"),
        (config.less, "less"),
        (config.scss, "scss"),
        (config.json, "json"),
        (config.html, "html"),
        (config.cocos2d, "plist"),
        (config.caat, "caat.json"),
        (config.css_vars, "vars.css"),
        (config.tailwind, "tailwind.json"),
        (config.dts, "d.ts"),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, ext)| format!("{name}.{ext}"))
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
/// (plain or cache-busted, see [`sprite_filename`]), text output or metadata file.
pub(crate) fn is_generated_file_name(file_name: &str, sprite_names: &[String]) -> bool {
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
            .find(|file| !config.output.join(file).exists());
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
        .find(|file| !config.output.join(file).exists());
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
/// `image_hash` and `image_file` are `None` when no image matching the current inputs
/// exists on disk (e.g. `--no-img` on a fresh output directory).
fn save_metadata(
    config: &Config,
    sprite: &Sprite,
    image_hash: Option<u64>,
    image_file: Option<&str>,
) {
    let meta_path = get_meta_path(config, sprite);
    let outputs_hash = compute_outputs_hash(config, sprite);
    let image_line = image_hash.map(|hash| hash.to_string()).unwrap_or_default();
    let image_file = image_file.unwrap_or_default();

    if let Ok(mut file) = File::create(&meta_path) {
        let _ = writeln!(file, "{image_line}\n{outputs_hash}\n{image_file}");
    }
}

fn generate_tailwind(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.tailwind.json", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };

    let class_names: Vec<String> = sprite
        .images
        .iter()
        .flat_map(|img| {
            std::iter::once(img.label())
                .chain(img.aliases.iter().map(|p| label_from_path(p)))
                .map(|lbl| build_class_name(config, &name, &lbl))
                .collect::<Vec<_>>()
        })
        .collect();

    let n = class_names.len();
    let _ = writeln!(f, "{{");
    let _ = writeln!(f, "  \"safelist\": [");
    for (i, cls) in class_names.iter().enumerate() {
        let comma = if i + 1 < n { "," } else { "" };
        let _ = writeln!(f, "    \"{}\"{}", cls, comma);
    }
    let _ = writeln!(f, "  ]");
    let _ = writeln!(f, "}}");
    log::info!("Generated Tailwind JSON: {:?}", output_path);
}

fn generate_dts(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let output_path = config.output.join(format!("{}.d.ts", name));
    let Some(mut f) = create_output(&output_path) else {
        return;
    };

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
        let _ = writeln!(f, "export type SpriteName = never;");
        return;
    }

    let _ = write!(f, "export type SpriteName");
    for (i, lbl) in labels.iter().enumerate() {
        let semi = if i + 1 == labels.len() { ";" } else { "" };
        let _ = writeln!(f, "\n  | \"{}\"{}", lbl, semi);
    }
    log::info!("Generated TypeScript definitions: {:?}", output_path);
}

fn print_dry_run(config: &Config, sprite: &Sprite) {
    let name = sprite.name();
    let (w, h) = sprite.dimensions();
    let total_entries: usize = sprite.images.iter().map(|i| 1 + i.aliases.len()).sum();

    println!(
        "Sprite: {} ({}×{} px, {} entries)",
        name, w, h, total_entries
    );
    for img in &sprite.images {
        for label in
            std::iter::once(img.label()).chain(img.aliases.iter().map(|p| label_from_path(p)))
        {
            println!(
                "  .{cls:40} at ({x},{y}) {w}×{h}px",
                cls = build_class_name(config, &name, &label),
                x = img.x,
                y = img.y,
                w = img.computed_width,
                h = img.computed_height,
            );
        }
    }
    println!("Would write to {}:", config.output.display());
    if !config.no_img {
        for file in sprite_filenames(config, sprite) {
            println!("  {file}");
        }
    }
    for file in text_output_names(config, &name) {
        println!("  {file}");
    }
}

pub fn generate_files(config: &Config, sprite: &Sprite) {
    if config.dry_run {
        print_dry_run(config, sprite);
        return;
    }

    let stored = read_metadata(config, sprite);
    let stale = staleness_from(config, sprite, &stored);
    if !stale.any() {
        log::info!("Sprite is up to date, skipping generation");
        return;
    }

    if stale.image {
        generate_sprite_image(config, sprite);
    }
    if stale.outputs {
        generate_text_outputs(config, sprite);
    }

    // With --no-img the image on disk is untouched, so keep what was previously stored.
    if config.no_img {
        save_metadata(
            config,
            sprite,
            stored.image_hash,
            stored.image_file.as_deref(),
        );
    } else {
        save_metadata(
            config,
            sprite,
            Some(compute_sprite_hash(config, sprite)),
            Some(&sprite_filename(config, sprite)),
        );
    }
}

fn generate_text_outputs(config: &Config, sprite: &Sprite) {
    use rayon::prelude::*;

    // Text-format outputs are all independent — generate them in parallel.
    let tasks: Vec<fn(&Config, &Sprite)> = {
        let mut t: Vec<fn(&Config, &Sprite)> = Vec::new();
        if !config.no_css {
            t.push(generate_css);
        }
        if config.html {
            t.push(generate_html);
        }
        if config.less {
            t.push(generate_less);
        }
        if config.scss {
            t.push(generate_scss);
        }
        if config.cocos2d {
            t.push(generate_coco2d);
        }
        if config.json {
            t.push(generate_json);
        }
        if config.caat {
            t.push(generate_caat);
        }
        if config.css_vars {
            t.push(generate_css_vars);
        }
        if config.tailwind {
            t.push(generate_tailwind);
        }
        if config.dts {
            t.push(generate_dts);
        }
        t
    };
    tasks.par_iter().for_each(|f| f(config, sprite));
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_css_pseudo_classes_constant() {
        // Verify known pseudo classes are present
        assert!(CSS_PSEUDO_CLASSES.contains(&"hover"));
        assert!(CSS_PSEUDO_CLASSES.contains(&"active"));
        assert!(CSS_PSEUDO_CLASSES.contains(&"focus"));
        assert_eq!(CSS_PSEUDO_CLASSES.len(), 10);
    }
}
