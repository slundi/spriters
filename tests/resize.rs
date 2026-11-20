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

    assert_eq!(img.computed_width, 120);
    assert_eq!(img.computed_height, 100);
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

    assert_eq!(img.computed_width, 200);
    assert_eq!(img.computed_height, 133); // floor(167 * 0.8) = floor(133.6) = 133
}
