use image::{GenericImage, RgbaImage};
use rand::{distr::Alphanumeric, RngExt};
use std::fs::File;
use std::io::{BufWriter, Write};

use crate::{Config, Sprite};

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

/// Get the filename, if there is cache buster, filename will change: if `config.cachebuster_filename = true` the cache buster
/// is in the filename like `icons_p3c54d.png` otherwise it is a a query param like `icons.png?p3c54d`
fn get_filename_depending_on_cache_buster(config: &Config, sprite: &Sprite) -> String {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let cache_buster = get_cache_buster(config.cachebuster, config.cachebuster_filename);
    format!(
        "{name}{cbf}.png{cbq}",
        name = name,
        cbf = if config.cachebuster_filename {
            cache_buster.clone()
        } else {
            String::with_capacity(0)
        },
        cbq = if config.cachebuster {
            cache_buster
        } else {
            String::with_capacity(0)
        }
    )
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
    let mut f = BufWriter::new(File::create(&sprite.path).expect("Unable to create file"));
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let _ = writeln!(
        &mut f,
        ".{name} {{\tbackground-image: url('{filename}'); background-repeat: no-repeat;\n}}",
        name = name,
        filename = get_filename_depending_on_cache_buster(config, sprite),
    );
    for img in sprite.images.iter() {
        // write!(&mut f, ".{}{} {{\n", it.1.label, it.1.pseudo);
        let _ = write!(&mut f, "\tbackground-position: {{ {x} ~ ('px' if {x}) }} {{ {y} ~ ('px' if {y}) }}; width: {w}px; height: {h}px;\n}}\n", x=img.x, y=img.y, w=img.computed_width, h=img.computed_height);
        //sprite.path.as_ref()
    }
    for r in config.ratios.iter() {
        let _ = writeln!(&mut f, "@media screen and (-webkit-min-device-pixel-ratio: {}), screen and (min--moz-device-pixel-ratio: {}), screen and (-o-min-device-pixel-ratio: {{ ratio.fraction }}), screen and (min-device-pixel-ratio: {}), screen and (min-resolution: {}dppx) {{", r, r, r, r);
        for it in sprite.images.iter().enumerate() {
            // write!(&mut f, "\t.{}{} {{\n", it.1.label, it.1.pseudo);
            let _ = write!(
                &mut f,
                "\t\tbackground-image: url('{}.png'); -webkit-background-size: {}px {}px; background-size: {}px {}px;\n\t}}\n",
                name,
                it.1.x, it.1.y, it.1.computed_width, it.1.computed_height
            );
        }
        let _ = write!(&mut f, "\t}}");
    }
}

fn generate_html(config: &Config, sprite: &Sprite) {
    let mut f = BufWriter::new(File::create(&sprite.path).expect("Unable to create file"));
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
    </tr></thead><tbody>", config.output.to_string_lossy(), n);
    for image in sprite.images.iter() {
        let _ = writeln!(
            &mut f,
            "<tr><td>.{label}</td><td><div class=\"{label}\"></div></td></tr>",
            label = image.label()
        );
    }
    let _ = write!(&mut f, "</tbody></table>
    <footer>Generated using <a href=\"https://codeberg.org/slundi/spriters\">Spriters v<%= version %></a></footer>
    </body></html>");
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
    let mut f = BufWriter::new(File::create(&sprite.path).expect("Unable to create file"));
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let _ = writeln!(
        f,
        ".{list} {{background-image: url('{name}');background-repeat: no-repeat;}}",
        list = implode_image_label(config, sprite).replace(",", ",."),
        name = name,
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
}

fn generate_scss(config: &Config, sprite: &Sprite) {
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let output_path = config.output.join(format!("{}.scss", name));
    let mut f = BufWriter::new(File::create(&output_path).expect("Unable to create SCSS file"));
    let (width, height) = sprite.dimensions();

    // Generate a mixin for the sprite base
    let _ = writeln!(
        f,
        "@mixin {name}-sprite {{\n  background-image: url('{name}.png');\n  background-repeat: no-repeat;\n  background-size: {w}px {h}px;\n}}",
        name = name,
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

fn generate_coco2d(_config: &Config, _sprite: &Sprite) {
    todo!();
}

fn generate_json(config: &Config, sprite: &Sprite) {
    let mut f = BufWriter::new(File::create(&sprite.path).expect("Unable to create file"));
    let name = sprite.path.file_name().unwrap().to_string_lossy();
    let n = sprite.images.len();
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
        "],\n\"meta\":{{\"width\": {w}, \"height\": {h}, \"sprite_filename\": \"{name}\", \"sprite_path\": \"{name}\"}}\n}}",
        w = 0, // TODO:
        h = 0,
        // TODO: image hash?
    );
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

    if let Err(e) = canvas.save(&output_path) {
        log::error!("Failed to save sprite image to {:?}: {}", output_path, e);
    } else {
        log::info!("Generated sprite: {:?} ({}x{})", output_path, width, height);
    }
}

pub fn generate_files(config: &Config, sprite: &Sprite) {
    if !config.no_img {
        generate_sprite_image(config, sprite);
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
    use super::get_cache_buster;

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
}
