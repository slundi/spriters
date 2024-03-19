use crate::{Algorithm, Sprite};

impl Algorithm {
    pub fn vertical(sprite: &mut Sprite) {
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = 0;
            image.y = y;
            y += image.absolute_height;
        }
    }
    pub fn horizontal(sprite: &mut Sprite) {
        let mut x = 0u32;
        for image in sprite.images.iter_mut() {
            image.y = 0;
            image.x = x;
            x += image.absolute_width;
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
            y += image.absolute_height;
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
            x += image.absolute_width;
        }
    }
    pub fn diagonal(sprite: &mut Sprite) {
        let mut x = 0u32;
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = x;
            image.y = y;
            x += image.absolute_width;
            y += image.absolute_height;
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
