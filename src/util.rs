use image::DynamicImage;

/// Return the total width of the image taking count of the margin, padding and ratio.
pub fn absolute_width(image: &DynamicImage, spacing: u32, ratio: f32) -> u32 {
    todo!()
}

pub fn round_up(value: f32) -> u32 {
    value.round().max(0f32) as u32
}

/// Return the nearest fraction. If fraction.Fraction is not available, return a fraction.
///
/// *Note: used for Opera CSS pixel-ratio media queries.*
pub fn nearest_fration(value: f32) -> String {
    let fraction = fraction::Fraction::from(value);
    format!("{}", fraction)
}
// def nearest_fration(value):
//     try:
//         from fraction import Fraction
//         return str(Fraction(value))
//     except ImportError:
//         return '%i/100' % int(float(value) * 100)

#[cfg(test)]
mod tests {
    use crate::util::{nearest_fration, round_up};

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(1.2), 1);
        assert_eq!(round_up(1.8), 2);
        assert_eq!(round_up(1.5), 2);
        assert_eq!(round_up(-5f32), 0);
        assert_eq!(round_up(f32::MAX), u32::MAX);
    }

    #[test]
    fn test_nearest_fraction() {
        assert_eq!(nearest_fration(1.2), "6/5");
        assert_eq!(nearest_fration(2.5), "5/2");
        assert_eq!(nearest_fration(0.), "0");
        assert_eq!(nearest_fration(0.5), "1/2");
        assert_eq!(nearest_fration(0.2), "1/5");
        assert_eq!(nearest_fration(0.8), "4/5");
    }
}