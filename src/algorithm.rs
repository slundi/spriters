use crate::{
    util::{absolute_length, horizontal_spacing, vertical_spacing},
    Algorithm, Sprite,
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
    #[test]
    fn test_foo_bar() {}
}
