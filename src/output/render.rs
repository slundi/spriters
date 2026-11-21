//! Rendering of the sprite images, one per ratio, and cleanup of the previous ones.

use image::{GenericImage, RgbaImage};
use std::io::Write;
use std::path::PathBuf;

use super::files::replace_file;
use super::names::{parse_sprite_image_name, sprite_filenames};
use super::png8::encode_png8;
use crate::{Config, Sprite, SpriteFormat};

/// Write the sprite image of every ratio. Returns whether all of them were written.
pub(super) fn generate_sprite_image(config: &Config, sprite: &Sprite) -> bool {
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
pub(super) fn encode_image(
    canvas: &RgbaImage,
    format: image::ImageFormat,
    writer: &mut (impl Write + std::io::Seek),
) -> std::io::Result<()> {
    canvas.write_to(writer, format).map_err(|e| match e {
        image::ImageError::IoError(e) => e,
        other => std::io::Error::other(other),
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
/// its default images (plain or cache-busted, see [`super::names::sprite_filename`], in any
/// format, not an `@<ratio>x` one), the most recently written. The others may be left from a
/// previous format or cache buster.
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
