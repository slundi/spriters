use spriters::{Config, Sprite};
use std::path::PathBuf;

fn europe_config() -> (Config, PathBuf) {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/images/europe");
    let config = Config {
        source: src.clone(),
        output: std::env::temp_dir(),
        ..Default::default()
    };
    (config, src)
}

// no.png is 120×100, fr.png is 250×167

#[test]
fn max_width_scales_image_proportionally() {
    let (mut config, src) = europe_config();
    config.max_width = Some(60); // exactly half of no.png's 120px width

    let sprite = Sprite::new(&config, src);
    let img = sprite
        .images
        .iter()
        .find(|i| i.path.ends_with("no.png"))
        .expect("no.png not found");

    assert_eq!(img.computed_width, 60);
    assert_eq!(img.computed_height, 50); // 100 * 0.5
}

#[test]
fn max_height_scales_image_proportionally() {
    let (mut config, src) = europe_config();
    config.max_height = Some(50); // exactly half of no.png's 100px height

    let sprite = Sprite::new(&config, src);
    let img = sprite
        .images
        .iter()
        .find(|i| i.path.ends_with("no.png"))
        .expect("no.png not found");

    assert_eq!(img.computed_width, 60); // 120 * 0.5
    assert_eq!(img.computed_height, 50);
}

#[test]
fn max_width_and_height_both_respected() {
    let (mut config, src) = europe_config();
    config.max_width = Some(200);
    config.max_height = Some(80);

    let sprite = Sprite::new(&config, src);
    for img in &sprite.images {
        assert!(
            img.computed_width <= 200,
            "{}: width {} > 200",
            img.path,
            img.computed_width
        );
        assert!(
            img.computed_height <= 80,
            "{}: height {} > 80",
            img.path,
            img.computed_height
        );
    }
}

#[test]
fn images_within_limit_are_not_resized() {
    // no.png is 120×100 — a limit well above both dimensions must leave it untouched
    let (mut config, src) = europe_config();
    config.max_width = Some(500);
    config.max_height = Some(500);

    let sprite = Sprite::new(&config, src);
    let img = sprite
        .images
        .iter()
        .find(|i| i.path.ends_with("no.png"))
        .expect("no.png not found");

    assert_eq!(img.computed_width, 120);
    assert_eq!(img.computed_height, 100);
}

#[test]
fn ratio_aware_does_not_resize_at_exact_effective_limit() {
    // no.png is 120×100; max_width=60, ratios=2,1 → effective = 60*2 = 120 = image width → no resize
    let (mut config, src) = europe_config();
    config.max_width = Some(60);
    config.ratios = Some("2,1".to_owned());

    let sprite = Sprite::new(&config, src);
    let img = sprite
        .images
        .iter()
        .find(|i| i.path.ends_with("no.png"))
        .expect("no.png not found");

    // Pixels stay at 2×; the layout size is in 1× (CSS pixel) units.
    assert_eq!((img.image.width(), img.image.height()), (120, 100));
    assert_eq!(img.computed_width, 60);
    assert_eq!(img.computed_height, 50);
}

#[test]
fn ratio_aware_resize_triggers_above_effective_limit() {
    // fr.png is 250×167; max_width=100, ratios=2,1 → effective = 200
    // 250 > 200 → scale = 200/250 = 0.8 → (200, ⌊167*0.8⌋) = (200, 133)
    let (mut config, src) = europe_config();
    config.max_width = Some(100);
    config.ratios = Some("2,1".to_owned());

    let sprite = Sprite::new(&config, src);
    let img = sprite
        .images
        .iter()
        .find(|i| i.path.ends_with("fr.png"))
        .expect("fr.png not found");

    assert_eq!((img.image.width(), img.image.height()), (200, 133)); // floor(167 * 0.8)
    // 1× size: 200 / 2 = 100, ceil(133 / 2) = 67
    assert_eq!(img.computed_width, 100);
    assert_eq!(img.computed_height, 67);
}

#[test]
fn crop_handles_content_off_center_vertically() {
    // Opaque 6×2 band at (3, 1) in a transparent 20×20 icon: far from the vertical centre.
    let source = std::env::temp_dir().join(format!(
        "spriters_crop_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&source).unwrap();
    let mut canvas = image::RgbaImage::new(20, 20);
    for y in 1..3 {
        for x in 3..9 {
            canvas.put_pixel(x, y, image::Rgba([0, 128, 255, 255]));
        }
    }
    canvas.save(source.join("band.png")).unwrap();

    let config = Config {
        source: source.clone(),
        output: std::env::temp_dir(),
        crop: true,
        ..Default::default()
    };
    let sprite = Sprite::new(&config, source.clone());
    std::fs::remove_dir_all(&source).ok();

    assert_eq!(sprite.images.len(), 1);
    assert_eq!(sprite.images[0].computed_width, 6);
    assert_eq!(sprite.images[0].computed_height, 2);
}
