//! Statistics of a generated sprite (`spriters stats`).

use std::path::{Path, PathBuf};

use image::DynamicImage;

use crate::output;

/// Per-sprite statistics derived from the generated sprite image.
#[derive(Debug)]
pub struct SpriteStats {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Size of the sprite image file on disk, in bytes.
    pub file_size: u64,
    /// Number of pixels with alpha > 0.
    pub used_pixels: u64,
    /// Total pixel count (width × height).
    pub total_pixels: u64,
}

impl SpriteStats {
    /// Packing efficiency as an integer percentage (0–100).
    pub fn packing_efficiency_pct(&self) -> u64 {
        if self.total_pixels == 0 {
            return 0;
        }
        self.used_pixels * 100 / self.total_pixels
    }
}

/// Why [`sprite_stats`] could not report on a sprite.
#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("no sprite image found")]
    NotFound,
    #[error("the sprite image recorded in the metadata is missing: {}", .0.display())]
    Missing(PathBuf),
    #[error("cannot read {}: {error}", .path.display())]
    Unreadable { path: PathBuf, error: String },
}

/// Load stats for a named sprite in `output_dir`.
///
/// Uses the image file recorded in the sprite metadata, which accounts for cache-busted
/// names and the image format; when it is missing, that is the error, since any other file
/// may be left from an older run. Metadata that records no image (older versions) falls
/// back to the most recently written sprite image of that name.
pub fn sprite_stats(output_dir: &Path, name: &str) -> Result<SpriteStats, StatsError> {
    let recorded = output::read_metadata_file(&output_dir.join(output::meta_file_name(name)))
        .image_file
        // A bare file name only: never follow a path out of the output directory.
        .filter(|file| Path::new(file).file_name() == Some(std::ffi::OsStr::new(file)));
    let img_path = match recorded {
        Some(file) => {
            let path = output_dir.join(file);
            if !path.is_file() {
                return Err(StatsError::Missing(path));
            }
            path
        }
        None => output::find_sprite_image(output_dir, name).ok_or(StatsError::NotFound)?,
    };

    let unreadable = |error: String| StatsError::Unreadable {
        path: img_path.clone(),
        error,
    };
    let file_size = std::fs::metadata(&img_path)
        .map_err(|e| unreadable(e.to_string()))?
        .len();
    let img = image::open(&img_path).map_err(|e| unreadable(e.to_string()))?;
    let (w, h) = (img.width(), img.height());
    let total = w as u64 * h as u64;

    Ok(SpriteStats {
        name: name.to_owned(),
        width: w,
        height: h,
        file_size,
        used_pixels: count_visible_pixels(&img, total),
        total_pixels: total,
    })
}

/// Number of non-transparent pixels of `image` (`total` when it has no alpha channel),
/// counted in the decoded buffer instead of a converted RGBA copy.
fn count_visible_pixels(image: &DynamicImage, total: u64) -> u64 {
    let visible = |alpha: &[u8], stride: usize| {
        alpha
            .iter()
            .step_by(stride)
            .filter(|&&value| value > 0)
            .count() as u64
    };
    match image {
        DynamicImage::ImageRgba8(buffer) => visible(buffer.as_raw().get(3..).unwrap_or(&[]), 4),
        DynamicImage::ImageLumaA8(buffer) => visible(buffer.as_raw().get(1..).unwrap_or(&[]), 2),
        DynamicImage::ImageRgba16(buffer) => {
            buffer.pixels().filter(|pixel| pixel.0[3] > 0).count() as u64
        }
        DynamicImage::ImageLumaA16(buffer) => {
            buffer.pixels().filter(|pixel| pixel.0[1] > 0).count() as u64
        }
        other if !other.color().has_alpha() => total,
        other => other
            .to_rgba8()
            .pixels()
            .filter(|pixel| pixel.0[3] > 0)
            .count() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_visible_pixels_matches_brute_force() {
        let brute_force = |image: &DynamicImage| {
            image
                .to_rgba8()
                .pixels()
                .filter(|pixel| pixel.0[3] > 0)
                .count() as u64
        };
        let rgba = image::RgbaImage::from_fn(13, 7, |x, y| {
            image::Rgba([x as u8, y as u8, 0, if (x * y) % 3 == 0 { 0 } else { 200 }])
        });
        let luma_alpha = image::GrayAlphaImage::from_fn(9, 5, |x, y| {
            image::LumaA([7, if x == y { 0 } else { 255 }])
        });
        let rgba16 = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_fn(6, 6, |x, _| {
            image::Rgba([0, 0, 0, if x < 2 { 0 } else { 65535 }])
        });
        for image in [
            DynamicImage::ImageRgba8(rgba),
            DynamicImage::ImageLumaA8(luma_alpha),
            DynamicImage::ImageRgba16(rgba16),
        ] {
            let total = image.width() as u64 * image.height() as u64;
            assert_eq!(count_visible_pixels(&image, total), brute_force(&image));
        }
        // Without alpha every pixel is visible.
        let rgb = DynamicImage::ImageRgb8(image::RgbImage::new(4, 3));
        assert_eq!(count_visible_pixels(&rgb, 12), 12);
    }
}
