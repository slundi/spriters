//! A sprite: its images, how they are ordered and placed, and its dimensions.

use std::path::{Path, PathBuf};

use image::{ColorType, DynamicImage};
use log::error;

use crate::decode::ImageCache;
use crate::labels::label_from_path;
use crate::util::{self, Spacing, absolute_length, horizontal_spacing, vertical_spacing};
use crate::{Algorithm, Config, Ordering};

#[derive(Debug, Default)]
pub struct ImageAndPosition {
    pub path: String,
    /// Pixels at the highest ratio (the source resolution).
    pub image: DynamicImage,
    /// Position in the sprite, in 1× (CSS pixel) units. During layout, top-left corner of the
    /// cell (margin included); afterwards, of the CSS box (after the margin).
    pub x: u32,
    pub y: u32,
    /// Size in 1× (CSS pixel) units: the pixel size divided by the highest ratio, rounded up
    /// (equal to the pixel size without `--ratios`). After layout, the padding is included:
    /// it is the size of the CSS box.
    pub computed_width: u32,
    pub computed_height: u32,
    /// Paths of images with identical content that consolidate into this region.
    pub aliases: Vec<String>,
    /// XxHash64 of the source file bytes, used to detect pixel changes between runs.
    pub source_hash: u64,
}

impl ImageAndPosition {
    pub fn label(&self) -> String {
        label_from_path(&self.path)
    }

    /// Where the image pixels start, in 1× units: inside the padding of its CSS box.
    pub fn pixel_origin(&self, padding: &Spacing) -> (u32, u32) {
        (self.x + padding.left, self.y + padding.top)
    }
}

#[derive(Debug, Default)]
pub struct Sprite {
    pub config: Config,
    pub path: PathBuf,
    pub ratios: Vec<f32>,
    pub images: Vec<ImageAndPosition>,
    /// Paths of the image files left out of the sprite: they could not be read or decoded,
    /// their label is already used by another image, or they duplicate another image with
    /// `--no-duplicate`.
    pub failed_images: Vec<String>,
    pub margin: Spacing,
    pub padding: Spacing,
    /// Hash of the images and their layout, computed on first use by the outputs (they all
    /// need it: staleness, metadata, cache busters). The layout never changes after
    /// [`Sprite::new`].
    pub(crate) layout_hash: std::sync::OnceLock<u64>,
}

/// Name of the sprite built from the directory `path`, used for every output file name.
///
/// Paths without a final component (`.`, `..`) are resolved first; a directory that still
/// has no name (`/`) falls back to `sprite`.
pub fn sprite_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| {
            path.canonicalize()
                .ok()?
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| String::from("sprite"))
}

impl Sprite {
    pub fn new(config: &Config, path: PathBuf) -> Sprite {
        Sprite::with_cache(config, path, None)
    }

    /// Like [`Sprite::new`], reusing the images of `cache` whose file did not change, and
    /// storing the newly decoded ones in it.
    pub fn with_cache(config: &Config, path: PathBuf, cache: Option<&ImageCache>) -> Sprite {
        let mut result = Sprite {
            config: config.clone(),
            path,
            margin: Spacing::from(config.margin.clone()),
            padding: Spacing::from(config.padding.clone()),
            ..Default::default()
        };
        // ratios: list & sort them (validated by `Config::check`, fall back to 1x otherwise)
        result.ratios = match config.ratios.as_deref().map(util::parse_ratios) {
            Some(Ok(ratios)) => ratios,
            Some(Err(e)) => {
                error!("{e}, using 1x");
                vec![1f32]
            }
            None => vec![1f32],
        };
        result.discover_images(cache);
        result.sort_images();
        result.process();
        result
    }

    /// Name of the sprite, see [`sprite_name`].
    pub fn name(&self) -> String {
        sprite_name(&self.path)
    }

    /// Whether the sprite holds every image found: none was left out (see
    /// [`Sprite::failed_images`]), and it is not empty.
    pub fn is_complete(&self) -> bool {
        !self.images.is_empty() && self.failed_images.is_empty()
    }

    /// Drop the decoded pixels of every image once the outputs are written; the layout
    /// (positions and sizes) is kept.
    pub fn release_pixels(&mut self) {
        for img in self.images.iter_mut() {
            img.image = DynamicImage::default();
        }
    }

    /// Generate sprite map
    fn process(&mut self) {
        match self.config.algorithm {
            Algorithm::Square => Algorithm::square(self),
            Algorithm::Vertical => Algorithm::vertical(self),
            Algorithm::Horizontal => Algorithm::horizontal(self),
            Algorithm::Diagonal => Algorithm::diagonal(self),
            Algorithm::VerticalRight => Algorithm::vertical_right(self),
            Algorithm::HorizontalBottom => Algorithm::horizontal_bottom(self),
        }
        self.cells_to_boxes();
    }

    /// The algorithms place cells (image + padding + margin) at `x`/`y`. Turn each cell into
    /// the box seen by CSS: it starts after the margin and includes the padding, so that
    /// `x`/`y`/`computed_*` are what every output describes. Pixels are drawn inside the
    /// padding (see [`ImageAndPosition::pixel_origin`]).
    fn cells_to_boxes(&mut self) {
        for img in self.images.iter_mut() {
            img.x += self.margin.left;
            img.y += self.margin.top;
            img.computed_width += self.padding.left + self.padding.right;
            img.computed_height += self.padding.top + self.padding.bottom;
        }
    }

    /// Sort images depending on the `config.ordering` value.
    ///
    /// All orderings use `path` as a tiebreaker so the result is a total order —
    /// no two elements compare as Equal. This guarantees byte-for-byte reproducible
    /// output regardless of the order images were collected from the filesystem.
    fn sort_images(&mut self) {
        let vspace = vertical_spacing(
            self.padding.top,
            self.padding.bottom,
            self.margin.top,
            self.margin.bottom,
        );
        let hspace = horizontal_spacing(
            self.padding.left,
            self.padding.right,
            self.margin.left,
            self.margin.right,
        );
        // Sizes and spacing are both in 1× (CSS pixel) units.
        let ratio = 1.0;

        self.images.sort_by(|a, b| {
            let primary = match self.config.ordering {
                Ordering::Maxside | Ordering::MaxsideReverse => {
                    let b_max = absolute_length(b.computed_height, vspace, ratio)
                        .max(absolute_length(b.computed_width, hspace, ratio));
                    let a_max = absolute_length(a.computed_height, vspace, ratio)
                        .max(absolute_length(a.computed_width, hspace, ratio));
                    b_max.cmp(&a_max)
                }
                Ordering::Height | Ordering::HeightReverse => {
                    let b_h = absolute_length(b.computed_height, vspace, ratio);
                    let a_h = absolute_length(a.computed_height, vspace, ratio);
                    b_h.cmp(&a_h)
                }
                Ordering::Width | Ordering::WidthReverse => {
                    let b_w = absolute_length(b.computed_width, hspace, ratio);
                    let a_w = absolute_length(a.computed_width, hspace, ratio);
                    b_w.cmp(&a_w)
                }
                Ordering::Area | Ordering::AreaReverse => {
                    let b_area = absolute_length(b.computed_width, hspace, ratio)
                        * absolute_length(b.computed_height, vspace, ratio);
                    let a_area = absolute_length(a.computed_width, hspace, ratio)
                        * absolute_length(a.computed_height, vspace, ratio);
                    b_area.cmp(&a_area)
                }
                Ordering::Filename | Ordering::FilenameReverse => Path::new(&b.path)
                    .file_name()
                    .unwrap()
                    .cmp(Path::new(&a.path).file_name().unwrap()),
            };
            // Tiebreaker: full path ensures a total order independent of collection order.
            primary.then_with(|| a.path.cmp(&b.path))
        });
        if self.config.ordering.is_reverse() {
            self.images.reverse();
        }
    }

    #[cfg(test)]
    fn get_max_color_type(&self) -> ColorType {
        let mut current = ColorType::L8;
        for i in self.images.iter() {
            current = get_greatest_quality(&current, &i.image.color());
        }
        current
    }

    /// Calculate the sprite dimensions based on positioned images, including the right and
    /// bottom margin of the last ones.
    pub fn dimensions(&self) -> (u32, u32) {
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        for img in self.images.iter() {
            width = width.max(img.x + img.computed_width + self.margin.right);
            height = height.max(img.y + img.computed_height + self.margin.bottom);
        }
        (width, height)
    }
}

/// Order color set by quality/number of colors/alpha
const SORTED_COLOR_TYPES: [ColorType; 10] = [
    ColorType::L8,
    ColorType::La8,
    ColorType::Rgb8,
    ColorType::Rgba8,
    ColorType::L16,
    ColorType::La16,
    ColorType::Rgb16,
    ColorType::Rgba16,
    ColorType::Rgb32F,
    ColorType::Rgba32F,
];

pub fn get_greatest_quality(a: &ColorType, b: &ColorType) -> ColorType {
    let mut index_a = 0usize;
    let mut index_b = 0usize;
    for c in SORTED_COLOR_TYPES.iter().enumerate() {
        if a == c.1 {
            index_a = c.0;
        }
        if b == c.1 {
            index_b = c.0;
        }
    }
    SORTED_COLOR_TYPES[index_a.max(index_b)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greatest_quality() {
        assert_eq!(
            get_greatest_quality(&ColorType::L8, &ColorType::Rgb16),
            ColorType::Rgb16
        );
        assert_eq!(
            get_greatest_quality(&ColorType::La8, &ColorType::La8),
            ColorType::La8
        );
        assert_eq!(
            get_greatest_quality(&ColorType::Rgba32F, &ColorType::Rgb16),
            ColorType::Rgba32F
        );
    }

    #[test]
    fn test_image_and_position_label() {
        let iap = ImageAndPosition {
            path: String::from("/path/to/my_image.png"),
            image: DynamicImage::new_rgba8(10, 10),
            x: 0,
            y: 0,
            computed_width: 10,
            computed_height: 10,
            ..Default::default()
        };
        assert_eq!(iap.label(), "my-image");

        // Test with spaces and special characters
        let iap2 = ImageAndPosition {
            path: String::from("/path/to/My Image File.png"),
            image: DynamicImage::new_rgba8(10, 10),
            x: 0,
            y: 0,
            computed_width: 10,
            computed_height: 10,
            ..Default::default()
        };
        assert_eq!(iap2.label(), "my-image-file");

        // Test with underscores
        let iap3 = ImageAndPosition {
            path: String::from("icon_arrow_left.png"),
            image: DynamicImage::new_rgba8(10, 10),
            x: 0,
            y: 0,
            computed_width: 10,
            computed_height: 10,
            ..Default::default()
        };
        assert_eq!(iap3.label(), "icon-arrow-left");
    }

    #[test]
    fn test_sprite_dimensions() {
        let config = Config::default();
        let mut sprite = Sprite {
            config: config.clone(),
            path: PathBuf::from("test"),
            ratios: vec![1.0],
            images: Vec::new(),
            margin: util::Spacing::default(),
            padding: util::Spacing::default(),
            failed_images: Vec::new(),
            layout_hash: Default::default(),
        };

        // Empty sprite should have zero dimensions
        assert_eq!(sprite.dimensions(), (0, 0));

        // Add an image at origin
        sprite.images.push(ImageAndPosition {
            path: String::from("test1.png"),
            image: DynamicImage::new_rgba8(50, 30),
            x: 0,
            y: 0,
            computed_width: 50,
            computed_height: 30,
            ..Default::default()
        });
        assert_eq!(sprite.dimensions(), (50, 30));

        // Add another image that extends the bounds
        sprite.images.push(ImageAndPosition {
            path: String::from("test2.png"),
            image: DynamicImage::new_rgba8(40, 40),
            x: 60,
            y: 20,
            computed_width: 40,
            computed_height: 40,
            ..Default::default()
        });
        // Width should be 60 + 40 = 100, Height should be 20 + 40 = 60
        assert_eq!(sprite.dimensions(), (100, 60));
    }

    #[test]
    fn test_sprite_name_never_panics() {
        assert_eq!(sprite_name(Path::new("icons")), "icons");
        assert_eq!(sprite_name(Path::new("assets/icons/")), "icons");
        // `.` and `..` have no final component: resolved through the real path.
        let cwd = std::env::current_dir().unwrap();
        let cwd_name = cwd.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(sprite_name(Path::new(".")), cwd_name);
        let parent_name = cwd
            .parent()
            .and_then(Path::file_name)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(sprite_name(Path::new("..")), parent_name);
        assert_eq!(sprite_name(Path::new("/")), "sprite");
    }

    #[test]
    fn test_get_max_color_type_promotes_to_highest() {
        let make_img_sprite = |images: Vec<DynamicImage>| Sprite {
            config: Config::default(),
            path: PathBuf::from("test"),
            ratios: vec![1.0],
            images: images
                .into_iter()
                .enumerate()
                .map(|(i, img)| ImageAndPosition {
                    path: format!("img{}.png", i),
                    image: img,
                    computed_width: 10,
                    computed_height: 10,
                    x: 0,
                    y: 0,
                    aliases: vec![],
                    source_hash: 0,
                })
                .collect(),
            margin: util::Spacing::default(),
            padding: util::Spacing::default(),
            failed_images: Vec::new(),
            layout_hash: Default::default(),
        };

        let s = make_img_sprite(vec![
            DynamicImage::new_luma8(10, 10),
            DynamicImage::new_rgb8(10, 10),
        ]);
        assert_eq!(s.get_max_color_type(), ColorType::Rgb8);

        let s2 = make_img_sprite(vec![
            DynamicImage::new_rgba8(10, 10),
            DynamicImage::new_rgb8(10, 10),
        ]);
        assert_eq!(s2.get_max_color_type(), ColorType::Rgba8);

        let s3 = make_img_sprite(vec![DynamicImage::new_luma8(10, 10)]);
        assert_eq!(s3.get_max_color_type(), ColorType::L8);
    }
}
