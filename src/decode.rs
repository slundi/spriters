//! Decoding and processing of the source images (SVG rasterising, resize, crop), and the
//! cache of decoded images kept between runs by `--watch`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use log::{debug, warn};

use crate::Config;

/// Hash of the options that shape a decoded image: an image cached with other values must
/// be processed again.
pub(crate) fn processing_key(config: &Config, display_ratio: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = twox_hash::XxHash64::with_seed(0);
    (config.max_width, config.max_height, config.crop).hash(&mut hasher);
    config.resize_algorithm.hash(&mut hasher);
    display_ratio.to_bits().hash(&mut hasher);
    hasher.finish()
}

/// Decode an image file and process it for the sprite: resize to `max_width`/`max_height`
/// (ratio-aware), then crop. Returns the RGBA8 image and the original size.
///
/// Resize happens before crop so that max-width/height apply to the full image (including
/// any transparent border), and crop removes the leftover padding after.
pub(crate) fn load_image(
    bytes: &[u8],
    path: &str,
    config: &Config,
    display_ratio: f32,
) -> Result<(DynamicImage, (u32, u32)), String> {
    let is_svg = path.to_lowercase().ends_with(".svg");
    // SVG: rasterized directly to the target size (no post-load resize needed).
    // Raster: loaded then resized if max-width/height requires it.
    let img = if is_svg {
        rasterize_svg(bytes, display_ratio, config.max_width, config.max_height)?
    } else {
        image::load_from_memory(bytes).map_err(|e| e.to_string())?
    };
    let original = (img.width(), img.height());
    let mut img = if is_svg {
        img
    } else {
        let (width, height) = get_dimensions_from_max(
            img.width(),
            img.height(),
            config.max_width,
            config.max_height,
            display_ratio,
        );
        if width != img.width() || height != img.height() {
            img.resize_exact(width, height, config.resize_algorithm)
        } else {
            img
        }
    };
    if config.crop {
        img = crop(img, None);
    }
    // Stored as RGBA8 once (free when it already is), so that the sprite rendering borrows
    // it instead of converting it per ratio.
    Ok((DynamicImage::ImageRgba8(img.into_rgba8()), original))
}

/// Images decoded by earlier runs (used by `--watch`): a file whose size and modification
/// time did not change is neither read nor decoded again.
///
/// Entries also record the options that shape the decoded image (max width/height, crop,
/// resize filter, highest ratio); an image processed with other values is decoded again.
#[derive(Debug, Default)]
pub struct ImageCache {
    entries: std::sync::Mutex<HashMap<PathBuf, CachedImage>>,
}

#[derive(Debug)]
pub(crate) struct CachedImage {
    pub(crate) modified: std::time::SystemTime,
    pub(crate) len: u64,
    pub(crate) processing: u64,
    pub(crate) source_hash: u64,
    /// Shared, so that a lookup only clones the pointer under the lock.
    pub(crate) image: std::sync::Arc<DynamicImage>,
}

impl ImageCache {
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, CachedImage>> {
        // A panic while holding the lock leaves a usable map: entries are inserted whole.
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Content hash and processed image of `path`, if cached for this exact file state.
    pub(crate) fn get(
        &self,
        path: &Path,
        modified: std::time::SystemTime,
        len: u64,
        processing: u64,
    ) -> Option<(u64, DynamicImage)> {
        let (source_hash, image) = {
            let entries = self.entries();
            let entry = entries.get(path)?;
            if entry.modified != modified || entry.len != len || entry.processing != processing {
                return None;
            }
            (entry.source_hash, std::sync::Arc::clone(&entry.image))
        };
        // The pixels are copied outside the lock, so parallel lookups do not wait on it.
        Some((source_hash, DynamicImage::clone(&image)))
    }

    pub(crate) fn insert(&self, path: PathBuf, image: CachedImage) {
        self.entries().insert(path, image);
    }

    /// Forget the images whose file no longer exists.
    pub fn retain_existing(&self) {
        self.entries().retain(|path, _| path.exists());
    }

    /// Number of cached images.
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    /// Whether no image is cached.
    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }
}

/// Rasterize an SVG at the appropriate pixel size.
///
/// The natural SVG size (in CSS px) is scaled by `display_ratio` for retina support,
/// then further constrained by `max_width`/`max_height` if set.
/// Returns a fully-decoded `DynamicImage` ready for the normal processing pipeline.
fn rasterize_svg(
    bytes: &[u8],
    display_ratio: f32,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<DynamicImage, String> {
    use resvg::{tiny_skia, usvg};

    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|e| format!("SVG parse error: {e}"))?;

    let natural = tree.size().to_int_size();
    let natural_w = natural.width();
    let natural_h = natural.height();

    // Scale natural size by display ratio so the rasterized image is crisp at the
    // target DPI (e.g. 2× for --ratios=2,1).
    let render_w = (natural_w as f32 * display_ratio).round() as u32;
    let render_h = (natural_h as f32 * display_ratio).round() as u32;

    // Apply max-dimension constraints at the scaled size.
    // Never zero: at least 1 pixel per side.
    let (target_w, target_h) =
        get_dimensions_from_max(render_w, render_h, max_width, max_height, display_ratio);

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h).ok_or("Failed to create pixmap")?;

    let scale_x = target_w as f32 / natural_w as f32;
    let scale_y = target_h as f32 / natural_h as f32;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );

    Ok(DynamicImage::ImageRgba8(pixmap_to_rgba(&pixmap)))
}

/// Straight-alpha RGBA image of a premultiplied tiny-skia pixmap: the same pixels as
/// `encode_png` followed by a PNG decode, without the zlib round trip.
fn pixmap_to_rgba(pixmap: &resvg::tiny_skia::Pixmap) -> image::RgbaImage {
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), rgba)
        .expect("a pixmap holds width × height RGBA pixels")
}

/// Result of cropping an image with nothing left to keep (fully transparent, or entirely
/// the crop colour): a 1×1 pixel taken from the image, so the icon keeps a sprite entry
/// (e.g. a placeholder) instead of producing a zero-sized one.
fn blank_placeholder(image: &DynamicImage) -> DynamicImage {
    warn!(
        "Image ({}x{}) has no pixel left after crop, keeping a 1x1 placeholder",
        image.width(),
        image.height()
    );
    image.crop_imm(0, 0, 1, 1)
}

/// Crop an image. When no color is provided, it crops white or transparent areas (when alpha channel is available).
///
/// An image with nothing to keep is reduced to a 1×1 placeholder.
pub fn crop(image: DynamicImage, color: Option<u32>) -> DynamicImage {
    let (crop_alpha, cropable_color) = match color {
        Some(color) => (false, color),
        None => (image.color().has_alpha(), 0xffffffff), // default is white
    };

    // Common 8-bit types are scanned in place, with the same RGBA conversion as
    // `GenericImageView::get_pixel`; other types are converted once.
    let keep =
        |rgba: [u8; 4]| !is_color_or_transparent(image::Rgba(rgba), crop_alpha, cropable_color);
    let bounds = match &image {
        DynamicImage::ImageRgba8(buffer) => content_bounds(buffer, |pixel| keep(pixel.0)),
        DynamicImage::ImageRgb8(buffer) => content_bounds(buffer, |pixel| {
            let [red, green, blue] = pixel.0;
            keep([red, green, blue, 255])
        }),
        DynamicImage::ImageLumaA8(buffer) => content_bounds(buffer, |pixel| {
            let [luma, alpha] = pixel.0;
            keep([luma, luma, luma, alpha])
        }),
        DynamicImage::ImageLuma8(buffer) => content_bounds(buffer, |pixel| {
            let [luma] = pixel.0;
            keep([luma, luma, luma, 255])
        }),
        other => content_bounds(&other.to_rgba8(), |pixel| keep(pixel.0)),
    };
    let Some((left, top, right, bottom)) = bounds else {
        return blank_placeholder(&image);
    };
    let (width, height) = (right - left + 1, bottom - top + 1);
    debug!(
        "\t\t{}x{}, left={}, top={} => {}x{}",
        image.width(),
        image.height(),
        left,
        top,
        width,
        height,
    );
    image.crop_imm(left, top, width, height)
}

/// Inclusive bounding box `(left, top, right, bottom)` of the pixels to `keep`, `None` when
/// there is none.
///
/// Only the croppable border is visited: rows are scanned from the top and from the bottom
/// until content is found, then each row in between from the left up to the current left
/// bound and from the right down to the current right bound.
fn content_bounds<P: image::Pixel>(
    buffer: &image::ImageBuffer<P, Vec<P::Subpixel>>,
    keep: impl Fn(&P) -> bool,
) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = buffer.dimensions();
    let row_has_content = |y: u32| (0..width).any(|x| keep(buffer.get_pixel(x, y)));

    let top = (0..height).find(|&y| row_has_content(y))?;
    let bottom = (top..height).rev().find(|&y| row_has_content(y))?;
    let mut left = width;
    let mut right: Option<u32> = None;
    for y in top..=bottom {
        if let Some(x) = (0..left).find(|&x| keep(buffer.get_pixel(x, y))) {
            left = x;
        }
        let start = right.map_or(0, |right| right + 1);
        if let Some(x) = (start..width).rev().find(|&x| keep(buffer.get_pixel(x, y))) {
            right = Some(x);
        }
    }
    Some((left, top, right?, bottom))
}

fn is_color_or_transparent(pixel: image::Rgba<u8>, crop_alpha: bool, color: u32) -> bool {
    if crop_alpha {
        return pixel.0[3] == 0;
    }
    u32::from_be_bytes(pixel.0) == color
}

/// Get the `(width, height)` tuple after applying max-width/max-height constraints.
///
/// `display_ratio` scales the limits: at ratio 2 the source is already 2× so the
/// effective ceiling is `max_width * display_ratio` (and similarly for height).
/// Pass `1.0` when ratios are not in use.
///
/// The limited side gets the (whole) limit; the other one is scaled with it and rounded to
/// the nearest pixel, never below 1: a 1000×1 image at max-width 10 is 10×1, not 10×0.
fn get_dimensions_from_max(
    width: u32,
    height: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
    display_ratio: f32,
) -> (u32, u32) {
    let ratio = display_ratio as f64;
    let (mut new_width, mut new_height) = (width as f64, height as f64);
    if let Some(max) = max_width {
        // Floored: the result must not exceed the limit.
        let limit = (max as f64 * ratio).floor();
        if limit < new_width {
            new_height *= limit / new_width;
            new_width = limit;
        }
    }
    if let Some(max) = max_height {
        let limit = (max as f64 * ratio).floor();
        if limit < new_height {
            new_width *= limit / new_height;
            new_height = limit;
        }
    }
    // The limits are whole numbers and the other side only shrinks, so rounding never
    // exceeds a limit (except to keep 1 pixel).
    let to_pixels = |length: f64| (length.round() as u32).max(1);
    (to_pixels(new_width), to_pixels(new_height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ColorType, GenericImageView};

    #[test]
    fn test_is_croppable_color() {
        // not croppable color
        // transparent one
        assert!(!is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 0]),
            false,
            0xffffff00
        ));
        // full opacity
        assert!(!is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 255]),
            false,
            0xffffff00
        ));

        // transparent but not set
        assert!(!is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 0]),
            false,
            0xffffff00
        ));

        // transparent cropable
        assert!(is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 0]),
            true,
            0xffffff00
        ));

        // croppable color no alpha
        assert!(is_color_or_transparent(
            image::Rgba::from([255u8, 255, 255, 255]),
            false,
            0xffffffff
        ));
    }

    #[test]
    fn test_max_dimensions() {
        // ratio 1.0 — identical to pre-ratio behaviour
        assert_eq!(get_dimensions_from_max(10, 20, None, None, 1.0), (10, 20));
        assert_eq!(get_dimensions_from_max(10, 20, Some(5), None, 1.0), (5, 10));
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(15), None, 1.0),
            (10, 20)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, None, Some(10), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, None, Some(40), 1.0),
            (10, 20)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(5), Some(10), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(5), Some(15), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(10), Some(10), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(2), Some(10), 1.0),
            (2, 4)
        );
        // 10×20 → 5×10 (max-width) → 2.5×5 (max-height): rounded, not truncated.
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(5), Some(5), 1.0),
            (3, 5)
        );
    }

    #[test]
    fn test_max_dimensions_never_zero() {
        // Review 6.7: the scaled side was truncated, so a very elongated image lost its
        // only row or column.
        assert_eq!(
            get_dimensions_from_max(1000, 1, Some(10), None, 1.0),
            (10, 1)
        );
        assert_eq!(
            get_dimensions_from_max(1, 1000, None, Some(10), 1.0),
            (1, 10)
        );
        assert_eq!(
            get_dimensions_from_max(1000, 1, Some(10), Some(10), 2.0),
            (20, 1)
        );
        // Even when the limit itself rounds down to nothing (max 1 at a 0.5 ratio).
        assert_eq!(get_dimensions_from_max(10, 10, Some(1), None, 0.5), (1, 1));
    }

    #[test]
    fn test_max_dimensions_round_to_nearest() {
        // Truncation gave the lower value in each case.
        // 8 × 10/30 = 2.67 → 3.
        assert_eq!(get_dimensions_from_max(30, 8, Some(10), None, 1.0), (10, 3));
        // 30 × 20/80 = 7.5 → 8.
        assert_eq!(
            get_dimensions_from_max(30, 80, None, Some(20), 1.0),
            (8, 20)
        );
        // 1.5 ratio: limit 100 × 1.5 = 150, 200 × 150/301 = 99.67 → 100.
        assert_eq!(
            get_dimensions_from_max(301, 200, Some(100), None, 1.5),
            (150, 100)
        );
    }

    #[test]
    fn test_max_dimensions_never_exceed_the_limit() {
        for width in 1..60 {
            for height in 1..60 {
                for max in 1..20 {
                    let (new_width, new_height) =
                        get_dimensions_from_max(width, height, Some(max), Some(max), 1.0);
                    assert!(new_width <= max.max(1) && new_height <= max.max(1));
                    assert!(new_width >= 1 && new_height >= 1);
                    assert!(new_width <= width && new_height <= height);
                }
            }
        }
    }

    #[test]
    fn test_max_dimensions_ratio_aware() {
        // ratio 2.0: effective max = limit * 2, so a 400px image is within a 200px limit at 2×
        assert_eq!(
            get_dimensions_from_max(400, 300, Some(200), None, 2.0),
            (400, 300)
        );
        // 500px > 200*2=400 → resize to 400; height = 300 * (400/500) = 240
        assert_eq!(
            get_dimensions_from_max(500, 300, Some(200), None, 2.0),
            (400, 240)
        );
        // max-height only: 600px > 100*2=200 → resize; width = 400 * (200/600) = 133
        assert_eq!(
            get_dimensions_from_max(400, 600, None, Some(100), 2.0),
            (133, 200)
        );
        // ratio 1.5: effective max = 100*1.5=150; 200 > 150 → resize to 150; h = 100*(150/200)=75
        assert_eq!(
            get_dimensions_from_max(200, 100, Some(100), None, 1.5),
            (150, 75)
        );
    }

    #[test]
    fn test_crop() {
        let img = image::open("tests/images/europe/no.png").unwrap();
        assert_eq!(img.width(), 120);
        assert_eq!(img.height(), 100);

        // crop transparency: must match the bounding box of non-transparent pixels
        let (bbox_width, bbox_height) = opaque_bounding_box(&img);
        let mut img2 = img.clone();
        img2 = crop(img2, None);
        assert_eq!((img2.width(), img2.height()), (bbox_width, bbox_height));
        assert_eq!((img2.width(), img2.height()), (100, 73));

        // crop white... will be same size
        let mut img2 = img.clone();
        img2 = crop(img2, Some(0xffffffff));
        assert_eq!(img2.width(), 120);
        assert_eq!(img2.height(), 100);
    }

    /// Reference implementation: size of the bounding box of all non-transparent pixels.
    fn opaque_bounding_box(image: &DynamicImage) -> (u32, u32) {
        let rgba = image.to_rgba8();
        let opaque: Vec<(u32, u32)> = rgba
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel.0[3] != 0)
            .map(|(x, y, _)| (x, y))
            .collect();
        let min_x = opaque.iter().map(|point| point.0).min().unwrap();
        let max_x = opaque.iter().map(|point| point.0).max().unwrap();
        let min_y = opaque.iter().map(|point| point.1).min().unwrap();
        let max_y = opaque.iter().map(|point| point.1).max().unwrap();
        (max_x - min_x + 1, max_y - min_y + 1)
    }

    /// Transparent `width`×`height` image with the given rectangles painted opaque red.
    fn transparent_with(width: u32, height: u32, opaque: &[(u32, u32, u32, u32)]) -> DynamicImage {
        let mut canvas = image::RgbaImage::new(width, height);
        for &(left, top, rect_width, rect_height) in opaque {
            for y in top..top + rect_height {
                for x in left..left + rect_width {
                    canvas.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
                }
            }
        }
        DynamicImage::ImageRgba8(canvas)
    }

    #[test]
    fn test_crop_single_row_near_top() {
        // Review 3.1: a single opaque row at y=1 gave 10x8 instead of 10x1.
        let cropped = crop(transparent_with(10, 10, &[(0, 1, 10, 1)]), None);
        assert_eq!((cropped.width(), cropped.height()), (10, 1));
    }

    #[test]
    fn test_crop_single_row_near_bottom() {
        let cropped = crop(transparent_with(10, 10, &[(0, 8, 10, 1)]), None);
        assert_eq!((cropped.width(), cropped.height()), (10, 1));
    }

    #[test]
    fn test_crop_off_center_block_keeps_only_content() {
        let cropped = crop(transparent_with(10, 10, &[(2, 1, 3, 2)]), None);
        assert_eq!((cropped.width(), cropped.height()), (3, 2));
        assert!(
            cropped.to_rgba8().pixels().all(|pixel| pixel.0[3] == 255),
            "only the opaque block must remain"
        );
    }

    #[test]
    fn test_crop_bounding_box_of_scattered_pixels() {
        // Two single pixels: the result is their bounding box, gaps included.
        let cropped = crop(transparent_with(12, 9, &[(3, 1, 1, 1), (7, 6, 1, 1)]), None);
        assert_eq!((cropped.width(), cropped.height()), (5, 6));
    }

    /// Reference bounding box `(left, top, width, height)` of the pixels `crop` must keep,
    /// found by testing every pixel; `None` when there is none.
    fn brute_force_bounds(
        image: &DynamicImage,
        color: Option<u32>,
    ) -> Option<(u32, u32, u32, u32)> {
        let (crop_alpha, cropable) = match color {
            Some(color) => (false, color),
            None => (image.color().has_alpha(), 0xffffffff),
        };
        let kept: Vec<(u32, u32)> = image
            .pixels()
            .filter(|(_, _, pixel)| !is_color_or_transparent(*pixel, crop_alpha, cropable))
            .map(|(x, y, _)| (x, y))
            .collect();
        let left = kept.iter().map(|point| point.0).min()?;
        let right = kept.iter().map(|point| point.0).max()?;
        let top = kept.iter().map(|point| point.1).min()?;
        let bottom = kept.iter().map(|point| point.1).max()?;
        Some((left, top, right - left + 1, bottom - top + 1))
    }

    #[test]
    fn test_crop_matches_brute_force_on_random_images() {
        let mut state: u32 = 0x9e37_79b9;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for case in 0..300 {
            let (width, height) = (1 + next() % 24, 1 + next() % 24);
            let mut canvas = image::RgbaImage::new(width, height);
            // From blank to dense; some cases draw a solid block instead of scattered pixels.
            let density = case % 7;
            for (_, _, pixel) in canvas.enumerate_pixels_mut() {
                if density > 0 && next() % 40 < density {
                    *pixel = image::Rgba([next() as u8, 0, 0, 1 + (next() % 255) as u8]);
                }
            }
            let image = DynamicImage::ImageRgba8(canvas);
            for color in [None, Some(0x0000_0000)] {
                let cropped = crop(image.clone(), color);
                match brute_force_bounds(&image, color) {
                    Some((left, top, width, height)) => {
                        assert_eq!(
                            cropped.to_rgba8(),
                            image.crop_imm(left, top, width, height).to_rgba8(),
                            "case {case}, color {color:?}"
                        );
                    }
                    None => assert_eq!(cropped.dimensions(), (1, 1), "case {case}: placeholder"),
                }
            }
        }
    }

    /// Semi-transparent gradient and shapes: demultiplication rounding matters here.
    const TRANSLUCENT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48">
  <defs><linearGradient id="g"><stop offset="0" stop-color="#f80" stop-opacity="0.1"/>
  <stop offset="1" stop-color="#08f" stop-opacity="0.9"/></linearGradient></defs>
  <rect width="64" height="48" fill="url(#g)"/>
  <circle cx="20" cy="24" r="15" fill="#3c3" fill-opacity="0.37"/>
</svg>"##;

    #[test]
    fn test_pixmap_to_rgba_matches_png_round_trip() {
        // Review 5.3: same pixels as the former `encode_png` + `load_from_memory`.
        use resvg::{tiny_skia, usvg};
        let tree =
            usvg::Tree::from_data(TRANSLUCENT_SVG.as_bytes(), &usvg::Options::default()).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(64, 48).unwrap();
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        let round_trip = image::load_from_memory(&pixmap.encode_png().unwrap())
            .unwrap()
            .to_rgba8();
        let direct = pixmap_to_rgba(&pixmap);
        assert_eq!(direct, round_trip);
        // The test is meaningful: there are semi-transparent pixels.
        assert!(
            direct
                .pixels()
                .any(|pixel| pixel.0[3] > 0 && pixel.0[3] < 255)
        );
    }

    #[test]
    fn test_rasterize_svg_returns_rgba8_at_the_target_size() {
        let image = rasterize_svg(TRANSLUCENT_SVG.as_bytes(), 1.0, None, None).unwrap();
        assert_eq!(image.color(), ColorType::Rgba8);
        assert_eq!(image.dimensions(), (64, 48));

        let retina = rasterize_svg(TRANSLUCENT_SVG.as_bytes(), 2.0, None, None).unwrap();
        assert_eq!(retina.dimensions(), (128, 96));
        let limited = rasterize_svg(TRANSLUCENT_SVG.as_bytes(), 1.0, Some(32), None).unwrap();
        assert_eq!(limited.dimensions(), (32, 24));
    }

    #[test]
    fn test_crop_keeps_the_image_type() {
        // No alpha channel: white is cropped.
        let mut rgb = image::RgbImage::from_pixel(12, 10, image::Rgb([255, 255, 255]));
        rgb.put_pixel(4, 3, image::Rgb([10, 20, 30]));
        rgb.put_pixel(7, 5, image::Rgb([10, 20, 30]));
        let cropped = crop(DynamicImage::ImageRgb8(rgb), None);
        assert_eq!(cropped.color(), ColorType::Rgb8);
        assert_eq!(cropped.dimensions(), (4, 3));

        // 16-bit images go through the RGBA conversion and keep their type.
        let mut rgba16 = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::new(9, 9);
        rgba16.put_pixel(2, 6, image::Rgba([65535, 0, 0, 65535]));
        let cropped = crop(DynamicImage::ImageRgba16(rgba16), None);
        assert_eq!(cropped.color(), ColorType::Rgba16);
        assert_eq!(cropped.dimensions(), (1, 1));

        // Grayscale with an explicit colour (white).
        let mut luma = image::GrayImage::from_pixel(8, 8, image::Luma([255]));
        luma.put_pixel(1, 1, image::Luma([0]));
        luma.put_pixel(2, 4, image::Luma([0]));
        let cropped = crop(DynamicImage::ImageLuma8(luma), Some(0xffffffff));
        assert_eq!(cropped.color(), ColorType::L8);
        assert_eq!(cropped.dimensions(), (2, 4));
    }

    #[test]
    fn test_crop_fully_transparent_gives_placeholder() {
        // Review 3.2: left = right = width overflowed `width - left - right`.
        let cropped = crop(transparent_with(10, 10, &[]), None);
        assert_eq!((cropped.width(), cropped.height()), (1, 1));
        assert_eq!(
            cropped.get_pixel(0, 0).0[3],
            0,
            "placeholder stays transparent"
        );
    }

    #[test]
    fn test_crop_transparent_single_column_or_row() {
        // One-pixel-wide images skip one axis: the other axis must not overflow either.
        let column = crop(transparent_with(1, 10, &[]), None);
        assert_eq!((column.width(), column.height()), (1, 1));
        let row = crop(transparent_with(10, 1, &[]), None);
        assert_eq!((row.width(), row.height()), (1, 1));
    }

    #[test]
    fn test_crop_single_pixel_is_unchanged() {
        let transparent = crop(transparent_with(1, 1, &[]), None);
        assert_eq!((transparent.width(), transparent.height()), (1, 1));
        let opaque = crop(transparent_with(1, 1, &[(0, 0, 1, 1)]), None);
        assert_eq!(opaque.get_pixel(0, 0), image::Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_crop_image_entirely_of_crop_color() {
        let canvas = image::RgbaImage::from_pixel(8, 6, image::Rgba([255, 255, 255, 255]));
        let cropped = crop(DynamicImage::ImageRgba8(canvas), Some(0xffffffff));
        assert_eq!((cropped.width(), cropped.height()), (1, 1));
        assert_eq!(cropped.get_pixel(0, 0), image::Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn test_crop_explicit_color() {
        let mut canvas = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 255, 255, 255]));
        canvas.put_pixel(7, 6, image::Rgba([0, 0, 0, 255]));
        let cropped = crop(DynamicImage::ImageRgba8(canvas), Some(0xffffffff));
        assert_eq!((cropped.width(), cropped.height()), (1, 1));
        assert_eq!(cropped.get_pixel(0, 0), image::Rgba([0, 0, 0, 255]));
    }
}
