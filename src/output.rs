use image::{GenericImage, RgbaImage};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use crate::{Config, JsonFormat, Sprite, SpriteFormat};

const SPRITERS_VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// Get the sprite image URL for use in CSS.
/// - If `config.url` is set, uses that as base URL
/// - Cache buster is derived deterministically from the sprite content hash.
fn get_sprite_url(config: &Config, sprite: &Sprite) -> String {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let ext = config.sprite_format.extension();

    // Deterministic cache buster: lower 32 bits of the sprite hash, hex-encoded.
    let buster: String = if config.cachebuster || config.cachebuster_filename {
        let h = compute_sprite_hash(config, sprite);
        let prefix = if config.cachebuster_filename {
            '_'
        } else {
            '?'
        };
        format!("{}{:08x}", prefix, h & 0xffff_ffff)
    } else {
        String::new()
    };

    let filename = format!(
        "{name}{cbf}.{ext}{cbq}",
        name = name,
        ext = ext,
        cbf = if config.cachebuster_filename {
            &buster
        } else {
            ""
        },
        cbq = if config.cachebuster && !config.cachebuster_filename {
            &buster
        } else {
            ""
        },
    );

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
    let sprite_name = sprite.path.file_name().unwrap().to_string_lossy();
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
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.css", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create CSS file"));
    let sprite_url = get_sprite_url(config, sprite);
    let (width, height) = sprite.dimensions();

    // Write the shared class for all images
    let _ = writeln!(
        &mut f,
        ".{list} {{\n  background-image: url('{url}');\n  background-repeat: no-repeat;\n}}",
        list = implode_image_label(config, sprite).replace(",", ", ."),
        url = sprite_url,
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

    // Write retina/ratio media queries if ratios are specified
    if let Some(ref ratios_str) = config.ratios {
        for ratio_str in ratios_str.split(',') {
            if let Ok(r) = ratio_str.trim().parse::<f32>() {
                if (r - 1.0).abs() < 0.001 {
                    continue; // Skip 1x ratio, it's the default
                }
                let fraction = crate::util::nearest_fration(r);
                let _ = writeln!(
                    &mut f,
                    "\n@media screen and (-webkit-min-device-pixel-ratio: {r}), screen and (min--moz-device-pixel-ratio: {r}), screen and (-o-min-device-pixel-ratio: {frac}), screen and (min-device-pixel-ratio: {r}), screen and (min-resolution: {r}dppx) {{",
                    r = r,
                    frac = fraction,
                );
                let _ = writeln!(
                    &mut f,
                    "  .{list} {{\n    background-image: url('{url}');\n    background-size: {w}px {h}px;\n  }}",
                    list = implode_image_label(config, sprite).replace(",", ", ."),
                    url = sprite_url,
                    w = width,
                    h = height,
                );
                let _ = writeln!(&mut f, "}}");
            }
        }
    }

    log::info!("Generated CSS: {:?}", output_path);
}

fn generate_html(config: &Config, sprite: &Sprite) {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.html", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create HTML file"));
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
            let _ = writeln!(
                &mut f,
                "<tr><td>.{label}</td><td><div class=\"{label}\"></div></td></tr>",
                label = label
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

fn generate_less_old(config: &Config, sprite: &Sprite) {
    let mut f = BufWriter::new(File::create(&sprite.path).expect("Unable to create file"));
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let _ = writeln!(
        f,
        "{list}{{
    background-image:url('{name}.png');
    -webkit-background-size: {w}px {h}px;
    -moz-background-size: {w}px {h}px;
    background-size: {w}px {h}px;
    @media screen and (-webkit-min-device-pixel-ratio: 1.0), screen and (min--moz-device-pixel-ratio: 1.0), screen and (-o-min-device-pixel-ratio: 100/100), screen and (min-device-pixel-ratio: 1.0),screen and (min-resolution: 1.0dppx){{
        background-image:url('{name}.png');
    }}
}}",
        list = implode_image_label(config, sprite),
        name = name,
        w = 0, // TODO: sprite.get_width(),
        h = 0, //sprite.get_height(),
    );
    for img in sprite.images.iter() {
        let class_name = build_class_name(config, &name, &img.label());
        let _ = writeln!(
            f,
            ".{class}{{background-position: {x} {y}; width: {w}px; height: {h}px;}}",
            class = class_name,
            x = img.x,
            y = img.y,
            w = img.computed_width,
            h = img.computed_height,
        );
    }
    let _ = writeln!(f, "@media screen and (-webkit-min-device-pixel-ratio: 1.0), screen and (min--moz-device-pixel-ratio: 1.0), screen and (-o-min-device-pixel-ratio: 100/100), screen and (min-device-pixel-ratio: 1.0), screen and (min-resolution: 1.0dppx){{
    background-image: url('{name}'); -webkit-background-size: {w}px {h}px; -moz-background-size: {w}px {h}px; background-size: {w}px {h}px;
}}",
        name = name,
        w = 0, // TODO:
        h = 0,
    );
}

fn generate_less(config: &Config, sprite: &Sprite) {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.less", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create LESS file"));
    let sprite_url = get_sprite_url(config, sprite);
    let _ = writeln!(
        f,
        ".{list} {{\n  background-image: url('{url}');\n  background-repeat: no-repeat;\n}}",
        list = implode_image_label(config, sprite).replace(",", ", ."),
        url = sprite_url,
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
            selector = selector, x = x, y = y, w = img.computed_width, h = img.computed_height,
        );
    }
    log::info!("Generated LESS: {:?}", output_path);
}

fn generate_scss(config: &Config, sprite: &Sprite) {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.scss", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create SCSS file"));
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
            selector = selector, x = img.x, y = img.y,
            w = img.computed_width, h = img.computed_height,
        );
    }
}

fn generate_css_vars(config: &Config, sprite: &Sprite) {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.vars.css", name));
    let mut f =
        BufWriter::new(File::create(&output_path).expect("Unable to create CSS vars file"));
    let sprite_url = get_sprite_url(config, sprite);

    let _ = writeln!(f, ":root {{");
    let _ = writeln!(f, "  --{name}-image: url('{url}');", name = name, url = sprite_url);

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

        for label in std::iter::once(img.label())
            .chain(img.aliases.iter().map(|p| label_from_path(p)))
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
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.plist", name));
    let mut f =
        BufWriter::new(File::create(&output_path).expect("Unable to create Cocos2d plist file"));
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
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.json", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create JSON file"));
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
                w = width, h = height, url = sprite_url,
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
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.caat.json", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create CAAT file"));
    let (width, height) = sprite.dimensions();
    let sprite_filename = format!("{}.{}", name, config.sprite_format.extension());
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

    let mut canvas = RgbaImage::new(width, height);

    for img in sprite.images.iter() {
        let rgba_img = img.image.to_rgba8();
        if let Err(e) = canvas.copy_from(&rgba_img, img.x, img.y) {
            log::error!("Failed to copy image {} to sprite: {}", img.path, e);
        }
    }

    let stem = sprite
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let ext = config.sprite_format.extension();
    let filename = if config.cachebuster_filename_only_sprites {
        let hash = compute_sprite_hash(config, sprite);
        format!("{stem}_{hash:x}.{ext}")
    } else {
        format!("{stem}.{ext}")
    };
    let output_path = config.output.join(filename);

    // png8 (grayscale) only applies to PNG; WebP and AVIF handle compression natively.
    let save_result = if config.png8 && config.sprite_format == SpriteFormat::Png {
        let grayscale = image::DynamicImage::ImageRgba8(canvas).to_luma8();
        grayscale.save(&output_path)
    } else {
        canvas.save(&output_path)
    };

    if let Err(e) = save_result {
        log::error!("Failed to save sprite image to {:?}: {}", output_path, e);
    } else {
        let mode = if config.png8 { "grayscale" } else { "RGBA" };
        log::info!(
            "Generated sprite: {:?} ({}x{}, {})",
            output_path,
            width,
            height,
            mode
        );
    }
}

/// Get the path to the metadata file for a sprite
fn get_meta_path(config: &Config, sprite: &Sprite) -> PathBuf {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    config.output.join(format!(".{}.spriters", name))
}

/// Compute a hash of all inputs that affect sprite generation
fn compute_sprite_hash(config: &Config, sprite: &Sprite) -> u64 {
    use std::hash::{Hash, Hasher};
    use twox_hash::XxHash64;

    let mut hasher = XxHash64::with_seed(0);

    // Hash version
    SPRITERS_VERSION.hash(&mut hasher);

    // Hash relevant config options
    config.sprite_format.extension().hash(&mut hasher);
    config.png8.hash(&mut hasher);
    config.crop.hash(&mut hasher);
    config.max_width.hash(&mut hasher);
    config.max_height.hash(&mut hasher);
    config.padding.hash(&mut hasher);
    config.margin.hash(&mut hasher);
    config.ratios.hash(&mut hasher);

    // Hash each source image path, layout position, dimensions, and aliases
    for img in &sprite.images {
        img.path.hash(&mut hasher);
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

/// Check if sprite needs regeneration based on cached metadata
pub(crate) fn should_regenerate(config: &Config, sprite: &Sprite) -> bool {
    if config.force {
        log::debug!("Force flag set, regenerating sprite");
        return true;
    }

    let meta_path = get_meta_path(config, sprite);
    let sprite_path = config.output.join(format!(
        "{}.{}",
        sprite
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        config.sprite_format.extension()
    ));

    // If sprite doesn't exist, regenerate
    if !sprite_path.exists() {
        log::debug!("Sprite file doesn't exist, generating");
        return true;
    }

    // If meta file doesn't exist, regenerate
    if !meta_path.exists() {
        log::debug!("Metadata file doesn't exist, regenerating");
        return true;
    }

    // Read stored hash from metadata file
    let stored_hash: Option<u64> = File::open(&meta_path).ok().and_then(|f| {
        let mut reader = BufReader::new(f);
        let mut contents = String::new();
        reader.read_to_string(&mut contents).ok()?;
        contents.trim().parse().ok()
    });

    match stored_hash {
        Some(hash) => {
            let current_hash = compute_sprite_hash(config, sprite);
            if hash == current_hash {
                log::info!("Sprite is up to date, skipping generation");
                false
            } else {
                log::debug!("Hash mismatch, regenerating sprite");
                true
            }
        }
        None => {
            log::debug!("Could not read metadata, regenerating");
            true
        }
    }
}

/// Save the sprite hash to metadata file
fn save_metadata(config: &Config, sprite: &Sprite) {
    let meta_path = get_meta_path(config, sprite);
    let hash = compute_sprite_hash(config, sprite);

    if let Ok(mut file) = File::create(&meta_path) {
        let _ = writeln!(file, "{}", hash);
    }
}

fn generate_tailwind(config: &Config, sprite: &Sprite) {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.tailwind.json", name));
    let mut f = BufWriter::new(
        File::create(&output_path).expect("Unable to create Tailwind JSON file"),
    );

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
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.d.ts", name));
    let mut f =
        BufWriter::new(File::create(&output_path).expect("Unable to create .d.ts file"));

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
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let (w, h) = sprite.dimensions();
    let total_entries: usize = sprite.images.iter().map(|i| 1 + i.aliases.len()).sum();

    println!("Sprite: {} ({}×{} px, {} entries)", name, w, h, total_entries);
    for img in &sprite.images {
        for label in std::iter::once(img.label())
            .chain(img.aliases.iter().map(|p| label_from_path(p)))
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
        println!("  {name}.{ext}", ext = config.sprite_format.extension());
    }
    if !config.no_css    { println!("  {name}.css"); }
    if config.less       { println!("  {name}.less"); }
    if config.scss       { println!("  {name}.scss"); }
    if config.json       { println!("  {name}.json"); }
    if config.html       { println!("  {name}.html"); }
    if config.cocos2d    { println!("  {name}.plist"); }
    if config.caat       { println!("  {name}.caat.json"); }
    if config.css_vars   { println!("  {name}.vars.css"); }
    if config.tailwind   { println!("  {name}.tailwind.json"); }
    if config.dts        { println!("  {name}.d.ts"); }
}

pub fn generate_files(config: &Config, sprite: &Sprite) {
    use rayon::prelude::*;

    if config.dry_run {
        print_dry_run(config, sprite);
        return;
    }

    // Check if sprite needs regeneration (unless generating only metadata files)
    if !config.no_img && !should_regenerate(config, sprite) {
        return;
    }

    // Image generation and metadata must happen sequentially (metadata depends on image).
    if !config.no_img {
        generate_sprite_image(config, sprite);
        save_metadata(config, sprite);
    }

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
            let sprite = Sprite {
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
                }],
                margin: crate::util::Spacing::default(),
                padding: crate::util::Spacing::default(),
            };
            sprite
        };

        // No cachebuster: URL is just the filename
        let config = Config::default();
        let sprite = make_sprite();
        let url = get_sprite_url(&config, &sprite);
        assert_eq!(url, "america.png");

        // cachebuster query param: deterministic (same hash every call)
        let mut config_q = Config::default();
        config_q.cachebuster = true;
        let url1 = get_sprite_url(&config_q, &make_sprite());
        let url2 = get_sprite_url(&config_q, &make_sprite());
        assert_eq!(url1, url2, "cache buster must be deterministic");
        assert!(url1.contains('?'), "query-param buster must contain '?'");

        // cachebuster filename: deterministic
        let mut config_f = Config::default();
        config_f.cachebuster_filename = true;
        let url3 = get_sprite_url(&config_f, &make_sprite());
        let url4 = get_sprite_url(&config_f, &make_sprite());
        assert_eq!(url3, url4, "filename buster must be deterministic");
        assert!(url3.contains('_'), "filename buster must contain '_'");
        assert!(url3.ends_with(".png"), "filename buster must end with .png");
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
