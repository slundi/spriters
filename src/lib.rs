pub mod algorithm;

/// Used to order the images before adding them to the canvas can be tunned. By default the algorithm is `Square``,
/// but in some situations using another ordering like vertical or horizontal could be useful depending on the kind of images you are spriting.
#[derive(Debug, Default)]
pub enum Algorithm {
    /// The square algorithm was inspired by the Binary Tree Bin Packing Algorithm Article by Jake Gordon.
    #[default]
    Square,
    /// The vertical one allocates the images vertically aligning them to the left of the sprite.
    Vertical,
    /// The vertical-right one allocates the images vertically aligning them to the right of the sprite.
    Hortizontal,
    /// The horizontal one allocates the images aligning them to the top of the sprite.
    Diagonal,
    /// The horizontal-bottom one allocates the images aligning them to the bottom of the sprite.
    VerticalRight,
    /// The diagonal one allocates the images diagonally. It was inspired by the Diagonal CSS Sprites Article by Aaron Barker.
    HorizontalBottom,
}

#[derive(Debug, Default)]
pub enum Ordering {
    #[default]
    Maxside,
    Width,
    Height,
    Area,
    Filename,
}

fn round_up<T>(value: T) -> T {
    value
}
// def round_up(value):
//     int_value = int(value)
//     diff = 1 if value > 0 else -1
//     return int_value + diff if value != int_value else int_value

/// Return the nearest fraction. If fraction.Fraction is not available, return a fraction.
///
/// Note: used for Opera CSS pixel-ratio media queries.
fn nearest_fration<T>(value: T) {}
// def nearest_fration(value):
//     try:
//         from fraction import Fraction
//         return str(Fraction(value))
//     except ImportError:
//         return '%i/100' % int(float(value) * 100)

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
