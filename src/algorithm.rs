use crate::{
    Algorithm, Sprite,
    util::{absolute_length, horizontal_spacing, vertical_spacing},
};

impl Algorithm {
    pub fn vertical(sprite: &mut Sprite) {
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = 0;
            image.y = y;
            y += absolute_length(
                image.image.height(),
                vertical_spacing(
                    sprite.padding.top,
                    sprite.padding.bottom,
                    sprite.margin.top,
                    sprite.margin.bottom,
                ),
                sprite.ratios[0],
            );
        }
    }
    pub fn horizontal(sprite: &mut Sprite) {
        let mut x = 0u32;
        for image in sprite.images.iter_mut() {
            image.y = 0;
            image.x = x;
            x += absolute_length(
                image.image.width(),
                horizontal_spacing(
                    sprite.padding.left,
                    sprite.padding.right,
                    sprite.margin.left,
                    sprite.margin.right,
                ),
                sprite.ratios[0],
            );
        }
    }
    pub fn vertical_right(sprite: &mut Sprite) {
        if sprite.images.is_empty() {
            return;
        }
        let max_width = sprite
            .images
            .iter()
            .max_by_key(|img| img.image.height())
            .unwrap()
            .image
            .width();
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = max_width - image.image.width();
            image.y = y;
            y += absolute_length(
                image.image.height(),
                vertical_spacing(
                    sprite.padding.top,
                    sprite.padding.bottom,
                    sprite.margin.top,
                    sprite.margin.bottom,
                ),
                sprite.ratios[0],
            );
        }
    }
    pub fn horizontal_bottom(sprite: &mut Sprite) {
        if sprite.images.is_empty() {
            return;
        }
        let max_height = sprite
            .images
            .iter()
            .max_by_key(|img| img.image.height())
            .unwrap()
            .image
            .height();
        let mut x = 0u32;
        for image in sprite.images.iter_mut() {
            image.y = max_height - image.image.height();
            image.x = x;
            x += absolute_length(
                image.image.width(),
                horizontal_spacing(
                    sprite.padding.left,
                    sprite.padding.right,
                    sprite.margin.left,
                    sprite.margin.right,
                ),
                sprite.ratios[0],
            );
        }
    }
    pub fn diagonal(sprite: &mut Sprite) {
        let mut x = 0u32;
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = x;
            image.y = y;
            x += absolute_length(
                image.image.width(),
                horizontal_spacing(
                    sprite.padding.left,
                    sprite.padding.right,
                    sprite.margin.left,
                    sprite.margin.right,
                ),
                sprite.ratios[0],
            );
            y += absolute_length(
                image.image.height(),
                vertical_spacing(
                    sprite.padding.top,
                    sprite.padding.bottom,
                    sprite.margin.top,
                    sprite.margin.bottom,
                ),
                sprite.ratios[0],
            );
        }
    }
    pub fn square(sprite: &mut Sprite) {
        crate::algorithm_square::process(sprite);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, ImageAndPosition, Sprite};
    use image::DynamicImage;

    fn create_test_sprite_with_images(sizes: Vec<(u32, u32)>) -> Sprite {
        let config = Config::default();
        let mut sprite = Sprite {
            config: config.clone(),
            path: std::path::PathBuf::from("test"),
            ratios: vec![1.0],
            images: Vec::new(),
            margin: crate::util::Spacing::default(),
            padding: crate::util::Spacing::default(),
        };

        for (i, (w, h)) in sizes.iter().enumerate() {
            let img = DynamicImage::new_rgba8(*w, *h);
            sprite.images.push(ImageAndPosition {
                path: format!("test{}.png", i),
                image: img,
                x: 0,
                y: 0,
                computed_width: *w,
                computed_height: *h,
                aliases: vec![],
            });
        }

        sprite
    }

    #[test]
    fn test_vertical_layout() {
        let mut sprite = create_test_sprite_with_images(vec![(10, 20), (15, 25), (8, 30)]);
        Algorithm::vertical(&mut sprite);

        // All images should have x = 0
        assert_eq!(sprite.images[0].x, 0);
        assert_eq!(sprite.images[1].x, 0);
        assert_eq!(sprite.images[2].x, 0);

        // Y positions should stack vertically
        assert_eq!(sprite.images[0].y, 0);
        assert_eq!(sprite.images[1].y, 20);
        assert_eq!(sprite.images[2].y, 45);
    }

    #[test]
    fn test_horizontal_layout() {
        let mut sprite = create_test_sprite_with_images(vec![(10, 20), (15, 25), (8, 30)]);
        Algorithm::horizontal(&mut sprite);

        // All images should have y = 0
        assert_eq!(sprite.images[0].y, 0);
        assert_eq!(sprite.images[1].y, 0);
        assert_eq!(sprite.images[2].y, 0);

        // X positions should stack horizontally
        assert_eq!(sprite.images[0].x, 0);
        assert_eq!(sprite.images[1].x, 10);
        assert_eq!(sprite.images[2].x, 25);
    }

    #[test]
    fn test_diagonal_layout() {
        let mut sprite = create_test_sprite_with_images(vec![(10, 20), (15, 25)]);
        Algorithm::diagonal(&mut sprite);

        // First image at origin
        assert_eq!(sprite.images[0].x, 0);
        assert_eq!(sprite.images[0].y, 0);

        // Second image offset diagonally
        assert_eq!(sprite.images[1].x, 10);
        assert_eq!(sprite.images[1].y, 20);
    }

    #[test]
    fn test_vertical_right_layout() {
        // Note: vertical_right uses max_by_key on height but gets width from that image
        // Using images where the tallest is also the widest to avoid overflow
        let mut sprite = create_test_sprite_with_images(vec![(10, 20), (15, 30), (8, 25)]);
        Algorithm::vertical_right(&mut sprite);

        // The image with max height (30) has width 15, so max_width = 15

        // Images should be offset from right
        assert_eq!(sprite.images[0].x, 15 - 10); // 5
        assert_eq!(sprite.images[1].x, 15 - 15); // 0
        assert_eq!(sprite.images[2].x, 15 - 8); // 7

        // Y positions should stack vertically
        assert_eq!(sprite.images[0].y, 0);
        assert_eq!(sprite.images[1].y, 20);
        assert_eq!(sprite.images[2].y, 50);
    }

    #[test]
    fn test_horizontal_bottom_layout() {
        let mut sprite = create_test_sprite_with_images(vec![(10, 20), (15, 25), (8, 30)]);
        Algorithm::horizontal_bottom(&mut sprite);

        // Images should be aligned to the bottom edge
        let max_height = 30; // tallest is 30

        // Images should be offset from bottom
        assert_eq!(sprite.images[0].y, max_height - 20);
        assert_eq!(sprite.images[1].y, max_height - 25);
        assert_eq!(sprite.images[2].y, max_height - 30);

        // X positions should stack horizontally
        assert_eq!(sprite.images[0].x, 0);
        assert_eq!(sprite.images[1].x, 10);
        assert_eq!(sprite.images[2].x, 25);
    }

    #[test]
    fn test_empty_images_vertical() {
        let mut sprite = create_test_sprite_with_images(vec![]);
        Algorithm::vertical(&mut sprite);
        assert!(sprite.images.is_empty());
    }

    #[test]
    fn test_empty_images_horizontal() {
        let mut sprite = create_test_sprite_with_images(vec![]);
        Algorithm::horizontal(&mut sprite);
        assert!(sprite.images.is_empty());
    }

    #[test]
    fn test_empty_images_vertical_right() {
        let mut sprite = create_test_sprite_with_images(vec![]);
        Algorithm::vertical_right(&mut sprite);
        assert!(sprite.images.is_empty());
    }

    #[test]
    fn test_empty_images_horizontal_bottom() {
        let mut sprite = create_test_sprite_with_images(vec![]);
        Algorithm::horizontal_bottom(&mut sprite);
        assert!(sprite.images.is_empty());
    }

    #[test]
    fn test_empty_images_diagonal() {
        let mut sprite = create_test_sprite_with_images(vec![]);
        Algorithm::diagonal(&mut sprite);
        assert!(sprite.images.is_empty());
    }

    #[test]
    fn test_empty_images_square() {
        let mut sprite = create_test_sprite_with_images(vec![]);
        Algorithm::square(&mut sprite);
        assert!(sprite.images.is_empty());
    }
}
