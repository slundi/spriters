use spriters::util::{Spacing, absolute_length, horizontal_spacing, vertical_spacing};
use spriters::{Algorithm, Config, Ordering, Sprite};
use std::path::{Path, PathBuf};

const ALGORITHMS: [Algorithm; 6] = [
    Algorithm::Square,
    Algorithm::Vertical,
    Algorithm::Horizontal,
    Algorithm::Diagonal,
    Algorithm::VerticalRight,
    Algorithm::HorizontalBottom,
];

const ORDERINGS: [Ordering; 10] = [
    Ordering::Maxside,
    Ordering::MaxsideReverse,
    Ordering::Width,
    Ordering::WidthReverse,
    Ordering::Height,
    Ordering::HeightReverse,
    Ordering::Area,
    Ordering::AreaReverse,
    Ordering::Filename,
    Ordering::FilenameReverse,
];

/// Image sizes mixing tiny, wide, tall and large blocks.
const SIZES: [(u32, u32); 9] = [
    (4, 4),
    (100, 4),
    (4, 100),
    (32, 32),
    (16, 48),
    (48, 16),
    (64, 20),
    (20, 64),
    (7, 9),
];

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_layout_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(&tmp);
    std::fs::remove_dir_all(&tmp).ok();
}

fn make_source(root: &Path) -> PathBuf {
    let source = root.join("square");
    std::fs::create_dir_all(&source).unwrap();
    for (index, &(width, height)) in SIZES.iter().enumerate() {
        // Distinct colours so no image is deduplicated as an alias.
        let pixel = image::Rgba([index as u8 * 20, 0, 255 - index as u8, 255]);
        image::RgbaImage::from_pixel(width, height, pixel)
            .save(source.join(format!("img{index}.png")))
            .unwrap();
    }
    source
}

/// Rectangle occupied by an image, padding and margin included.
fn footprints(sprite: &Sprite) -> Vec<(String, u32, u32, u32, u32)> {
    let padding = Spacing::from(sprite.config.padding.clone());
    let margin = Spacing::from(sprite.config.margin.clone());
    let hspace = horizontal_spacing(padding.left, padding.right, margin.left, margin.right);
    let vspace = vertical_spacing(padding.top, padding.bottom, margin.top, margin.bottom);
    sprite
        .images
        .iter()
        .map(|img| {
            (
                img.path.clone(),
                img.x,
                img.y,
                absolute_length(img.image.width(), hspace, sprite.ratios[0]),
                absolute_length(img.image.height(), vspace, sprite.ratios[0]),
            )
        })
        .collect()
}

fn assert_no_overlap(sprite: &Sprite, context: &str) {
    let rects = footprints(sprite);
    for (index, first) in rects.iter().enumerate() {
        for second in &rects[index + 1..] {
            let disjoint = first.1 + first.3 <= second.1
                || second.1 + second.3 <= first.1
                || first.2 + first.4 <= second.2
                || second.2 + second.4 <= first.2;
            assert!(
                disjoint,
                "[{context}] {} at ({},{}) overlaps {} at ({},{})",
                first.0, first.1, first.2, second.0, second.1, second.2
            );
        }
    }
}

/// The canvas must stay within a generous bound: a wrapped-around coordinate (unsigned
/// underflow in release builds) would produce a multi-gigapixel sprite.
fn assert_within_canvas(sprite: &Sprite, context: &str) {
    let (width, height) = sprite.dimensions();
    let bound = SIZES
        .iter()
        .map(|(img_width, img_height)| img_width + img_height)
        .sum::<u32>()
        * 4;
    assert!(
        width <= bound && height <= bound,
        "[{context}] canvas {width}x{height} is unreasonably large"
    );
}

#[test]
fn no_algorithm_overlaps_for_any_ordering_and_spacing() {
    with_tmp_dir(|tmp| {
        let source = make_source(tmp);
        let spacings: [(Option<&str>, Option<&str>); 3] =
            [(None, None), (Some("2"), None), (Some("1 3"), Some("2"))];

        for algorithm in ALGORITHMS {
            for ordering in ORDERINGS {
                for (padding, margin) in spacings {
                    let config = Config {
                        source: source.clone(),
                        algorithm,
                        ordering,
                        padding: padding.map(String::from),
                        margin: margin.map(String::from),
                        ..Default::default()
                    };

                    let sprite = Sprite::new(&config, source.clone());
                    let context = format!(
                        "{algorithm:?}, {ordering:?}, padding={padding:?}, margin={margin:?}"
                    );
                    assert_eq!(sprite.images.len(), SIZES.len());
                    assert_no_overlap(&sprite, &context);
                    assert_within_canvas(&sprite, &context);
                }
            }
        }
    });
}

#[test]
fn aligned_layouts_share_an_edge() {
    with_tmp_dir(|tmp| {
        let source = make_source(tmp);
        let widest = SIZES.iter().map(|size| size.0).max().unwrap();
        let tallest = SIZES.iter().map(|size| size.1).max().unwrap();

        for ordering in ORDERINGS {
            let config = Config {
                source: source.clone(),
                algorithm: Algorithm::VerticalRight,
                ordering,
                ..Default::default()
            };
            let sprite = Sprite::new(&config, source.clone());
            for img in &sprite.images {
                assert_eq!(
                    img.x + img.computed_width,
                    widest,
                    "[VerticalRight, {ordering:?}] {} not right-aligned",
                    img.path
                );
            }

            let config = Config {
                source: source.clone(),
                algorithm: Algorithm::HorizontalBottom,
                ordering,
                ..Default::default()
            };
            let sprite = Sprite::new(&config, source.clone());
            for img in &sprite.images {
                assert_eq!(
                    img.y + img.computed_height,
                    tallest,
                    "[HorizontalBottom, {ordering:?}] {} not bottom-aligned",
                    img.path
                );
            }
        }
    });
}

#[test]
fn square_default_ordering_is_reasonably_dense() {
    with_tmp_dir(|tmp| {
        let source = make_source(tmp);
        let config = Config {
            source: source.clone(),
            algorithm: Algorithm::Square,
            ..Default::default()
        };

        let sprite = Sprite::new(&config, source);
        let (width, height) = sprite.dimensions();
        let used: u32 = SIZES
            .iter()
            .map(|(img_width, img_height)| img_width * img_height)
            .sum();
        let density = used as f64 / (width * height) as f64;
        assert!(
            density >= 0.5,
            "density {density:.2} too low for a {width}x{height} sprite"
        );
    });
}
