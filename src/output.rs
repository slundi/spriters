use image::{GenericImage, RgbaImage};
use rand::{distr::Alphanumeric, RngExt};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use crate::{Config, Sprite};

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

/// Get the cache buster String when enabled.
/// If `filename = true` the cache buster is in the filename like `_p3c54d` otherwise it is a a query param like `?p3c54d`
fn get_cache_buster(enabled: bool, filename: bool) -> String {
    let mut cache_buster: String = String::with_capacity(7);
    if enabled {
        cache_buster.push(if filename { '_' } else { '?' });
        let hash: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(6)
            .map(char::from)
            .collect();
        cache_buster.push_str(&hash);
    }
    cache_buster
}

/// Get the sprite image URL for use in CSS.
/// - If `config.url` is set, uses that as base URL
/// - Handles cache buster in filename or as query param
fn get_sprite_url(config: &Config, sprite: &Sprite) -> String {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let cache_buster = get_cache_buster(config.cachebuster, config.cachebuster_filename);

    let filename = format!(
        "{name}{cbf}.png{cbq}",
        name = name,
        cbf = if config.cachebuster_filename {
            cache_buster.clone()
        } else {
            String::new()
        },
        cbq = if config.cachebuster && !config.cachebuster_filename {
            cache_buster
        } else {
            String::new()
        }
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

fn implode_image_label(config: &Config, sprite: &Sprite) -> String {
    let sprite_name = sprite.path.file_name().unwrap().to_string_lossy();
    let mut labels: Vec<String> = Vec::with_capacity(sprite.images.len());
    for img in sprite.images.iter() {
        labels.push(build_class_name(config, &sprite_name, &img.label()));
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

    // Write individual image classes
    for img in sprite.images.iter() {
        let class_name = build_class_name(config, &name, &img.label());
        let x = if img.x == 0 { String::from("0") } else { format!("-{}px", img.x) };
        let y = if img.y == 0 { String::from("0") } else { format!("-{}px", img.y) };
        let _ = writeln!(
            &mut f,
            "\n.{class} {{\n  background-position: {x} {y};\n  width: {w}px;\n  height: {h}px;\n}}",
            class = class_name,
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
    let n = sprite.images.len();
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
    </tr></thead><tbody>", name, n);
    for image in sprite.images.iter() {
        let _ = writeln!(
            &mut f,
            "<tr><td>.{label}</td><td><div class=\"{label}\"></div></td></tr>",
            label = image.label()
        );
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
        let class_name = build_class_name(config, &name, &img.label());
        let x = if img.x == 0 { String::from("0") } else { format!("-{}px", img.x) };
        let y = if img.y == 0 { String::from("0") } else { format!("-{}px", img.y) };
        let _ = writeln!(
            f,
            "\n.{class} {{\n  background-position: {x} {y};\n  width: {w}px;\n  height: {h}px;\n}}",
            class = class_name,
            x = x,
            y = y,
            w = img.computed_width,
            h = img.computed_height,
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

    // Generate individual image classes
    for img in sprite.images.iter() {
        let class_name = build_class_name(config, &name, &img.label());
        let _ = writeln!(
            f,
            "\n.{class} {{\n  background-position: -{x}px -{y}px;\n  width: {w}px;\n  height: {h}px;\n}}",
            class = class_name,
            x = img.x,
            y = img.y,
            w = img.computed_width,
            h = img.computed_height,
        );
    }
}

fn generate_coco2d(config: &Config, sprite: &Sprite) {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.plist", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create Cocos2d plist file"));
    let (width, height) = sprite.dimensions();
    let sprite_url = get_sprite_url(config, sprite);

    // Write plist header
    let _ = writeln!(f, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(f, r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">"#);
    let _ = writeln!(f, r#"<plist version="1.0">"#);
    let _ = writeln!(f, "<dict>");

    // Write frames
    let _ = writeln!(f, "    <key>frames</key>");
    let _ = writeln!(f, "    <dict>");

    for img in sprite.images.iter() {
        let _ = writeln!(f, "        <key>{}</key>", img.label());
        let _ = writeln!(f, "        <dict>");
        let _ = writeln!(f, "            <key>frame</key>");
        let _ = writeln!(f, "            <string>{{{{{},{}}},{{{},{}}}}}</string>",
            img.x, img.y, img.computed_width, img.computed_height);
        let _ = writeln!(f, "            <key>offset</key>");
        let _ = writeln!(f, "            <string>{{0,0}}</string>");
        let _ = writeln!(f, "            <key>rotated</key>");
        let _ = writeln!(f, "            <false/>");
        let _ = writeln!(f, "            <key>sourceColorRect</key>");
        let _ = writeln!(f, "            <string>{{{{0,0}},{{{},{}}}}}</string>",
            img.computed_width, img.computed_height);
        let _ = writeln!(f, "            <key>sourceSize</key>");
        let _ = writeln!(f, "            <string>{{{},{}}}</string>",
            img.computed_width, img.computed_height);
        let _ = writeln!(f, "        </dict>");
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
    let n = sprite.images.len();
    let (width, height) = sprite.dimensions();
    let sprite_url = get_sprite_url(config, sprite);
    let _ = writeln!(f, "{{\"frames\":[");
    for (index, img) in sprite.images.iter().enumerate() {
        let _ = writeln!(
            f,
            "{{\"filename\": \"{label}\", \"frame\":{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}}}{coma}",
            label = img.label(),
            x = img.x,
            y = img.y,
            w = img.computed_width,
            h = img.computed_height,
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
    log::info!("Generated JSON: {:?}", output_path);
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

    let output_path = config.output.join(format!(
        "{}.png",
        sprite.path.file_name().unwrap_or_default().to_string_lossy()
    ));

    // Convert to grayscale if png8 is enabled
    let save_result = if config.png8 {
        let grayscale = image::DynamicImage::ImageRgba8(canvas).to_luma8();
        grayscale.save(&output_path)
    } else {
        canvas.save(&output_path)
    };

    if let Err(e) = save_result {
        log::error!("Failed to save sprite image to {:?}: {}", output_path, e);
    } else {
        let mode = if config.png8 { "grayscale" } else { "RGBA" };
        log::info!("Generated sprite: {:?} ({}x{}, {})", output_path, width, height, mode);
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
    config.png8.hash(&mut hasher);
    config.crop.hash(&mut hasher);
    config.max_width.hash(&mut hasher);
    config.max_height.hash(&mut hasher);
    config.padding.hash(&mut hasher);
    config.margin.hash(&mut hasher);
    config.ratios.hash(&mut hasher);

    // Hash each source image path and content
    for img in &sprite.images {
        img.path.hash(&mut hasher);
        // Hash the image dimensions and position
        img.computed_width.hash(&mut hasher);
        img.computed_height.hash(&mut hasher);
    }

    hasher.finish()
}

/// Check if sprite needs regeneration based on cached metadata
fn should_regenerate(config: &Config, sprite: &Sprite) -> bool {
    if config.force {
        log::debug!("Force flag set, regenerating sprite");
        return true;
    }

    let meta_path = get_meta_path(config, sprite);
    let sprite_path = config.output.join(format!(
        "{}.png",
        sprite.path.file_name().unwrap_or_default().to_string_lossy()
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
    let stored_hash: Option<u64> = File::open(&meta_path)
        .ok()
        .and_then(|f| {
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

pub fn generate_files(config: &Config, sprite: &Sprite) {
    // Check if sprite needs regeneration (unless generating only metadata files)
    if !config.no_img && !should_regenerate(config, sprite) {
        return;
    }

    if !config.no_img {
        generate_sprite_image(config, sprite);
        // Save metadata after successful image generation
        save_metadata(config, sprite);
    }
    if !config.no_css {
        generate_css(config, sprite);
    }
    if config.html {
        generate_html(config, sprite);
    }
    if config.less {
        generate_less(config, sprite);
    }
    if config.scss {
        generate_scss(config, sprite);
    }
    if config.cocos2d {
        generate_coco2d(config, sprite);
    }
    if config.json {
        generate_json(config, sprite);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_buster() {
        assert_eq!(get_cache_buster(false, false), "");
        assert_eq!(get_cache_buster(false, true), "");

        let cb = get_cache_buster(true, false);
        assert_eq!(cb.chars().count(), 7);
        assert!(cb.starts_with('?'));

        let cb = get_cache_buster(true, true);
        assert_eq!(cb.chars().count(), 7);
        assert!(cb.starts_with('_'));
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
