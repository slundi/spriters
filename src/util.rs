use fraction::ToPrimitive;
use image::DynamicImage;

/// Utility struct for margin and padding
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Spacing {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}
impl Spacing {
    pub fn from(s: &str) -> Spacing {
        let data: Vec<&str> = s.split([',', ' '].as_ref()).collect();
        if data.len() == 4 {
            Spacing {
                top: data[0].parse().unwrap(),
                right: data[1].parse().unwrap(),
                bottom: data[2].parse().unwrap(),
                left: data[3].parse().unwrap(),
            }
        } else if data.len() == 3 {
            let horizontal: u32 = data[1].parse().unwrap();
            Spacing {
                top: data[0].parse().unwrap(),
                right: horizontal,
                bottom: data[2].parse().unwrap(),
                left: horizontal,
            }
        } else if data.len() == 2 {
            let vertical: u32 = data[0].parse().unwrap();
            let horizontal: u32 = data[1].parse().unwrap();
            Spacing {
                top: vertical,
                right: horizontal,
                bottom: vertical,
                left: horizontal,
            }
        } else {
            let value: u32 = s.parse().unwrap();
            Spacing {
                left: value,
                right: value,
                top: value,
                bottom: value,
            }
        }
    }
}

/// Return the horizontal padding and margin for an image.
pub fn horizontal_spacing(
    padding_left: u32,
    padding_right: u32,
    margin_left: u32,
    margin_right: u32,
) -> u32 {
    padding_left + padding_right + margin_left + margin_right
}

/// Return the vertical padding and margin for an image.
pub fn vertical_spacing(
    padding_top: u32,
    padding_bottom: u32,
    margin_top: u32,
    margin_bottom: u32,
) -> u32 {
    padding_top + padding_bottom + margin_top + margin_bottom
}

/// Return the total width of the image taking count of the margin, padding and ratio.
pub fn absolute_width(image: &DynamicImage, horizontal_spacing: u32, ratio: f32) -> u32 {
    round_up(image.width().to_f32().unwrap() + horizontal_spacing.to_f32().unwrap() * ratio)
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
    use crate::util::*;

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

    #[test]
    fn test_spacing() {
        assert_eq!(Spacing::from("5"), Spacing { top: 5, right: 5, bottom: 5, left: 5 });
        assert_eq!(Spacing::from("5 8"), Spacing { top: 5, right: 8, bottom: 5, left: 8 });
        assert_eq!(Spacing::from("5 8,2"), Spacing { top: 5, right: 8, bottom: 2, left: 8 });
        assert_eq!(Spacing::from("1 2,3 4"), Spacing { top: 1, right: 2, bottom: 3, left: 4 });

        assert_eq!(horizontal_spacing(5, 5, 2, 2), 14);
        assert_eq!(vertical_spacing(1, 1, 2, 2), 6);
    }
}
