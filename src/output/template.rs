//! Custom output templates (`--css-template`…), rendered with minijinja.

use std::io::Write;
use std::path::PathBuf;

use super::TextWriter;
use super::classes::entries;
use super::names::{
    cache_buster, get_sprite_url, sprite_file_url, sprite_filename, sprite_filename_at,
};
use crate::{Config, Sprite};

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
pub(super) fn templated(
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
