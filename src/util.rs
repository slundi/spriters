use fraction::ToPrimitive;
use std::{ops, str::FromStr};

/// Utility struct for margin and padding
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Spacing {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}
impl FromStr for Spacing {
    type Err = ();
    fn from_str(s: &str) -> Result<Spacing, Self::Err> {
        let data: Vec<&str> = s.split([',', ' '].as_ref()).collect();
        if data.len() == 4 {
            Ok(Spacing {
                top: data[0].parse().unwrap(),
                right: data[1].parse().unwrap(),
                bottom: data[2].parse().unwrap(),
                left: data[3].parse().unwrap(),
            })
        } else if data.len() == 3 {
            let horizontal: u32 = data[1].parse().unwrap();
            Ok(Spacing {
                top: data[0].parse().unwrap(),
                right: horizontal,
                bottom: data[2].parse().unwrap(),
                left: horizontal,
            })
        } else if data.len() == 2 {
            let vertical: u32 = data[0].parse().unwrap();
            let horizontal: u32 = data[1].parse().unwrap();
            Ok(Spacing {
                top: vertical,
                right: horizontal,
                bottom: vertical,
                left: horizontal,
            })
        } else {
            let value: u32 = s.parse().unwrap();
            Ok(Spacing {
                left: value,
                right: value,
                top: value,
                bottom: value,
            })
        }
    }
}
impl ops::Add<Spacing> for Spacing {
    type Output = Spacing;

    fn add(self, rhs: Spacing) -> Self::Output {
        let mut result = self;
        result.top += rhs.top;
        result.right += rhs.right;
        result.bottom += rhs.bottom;
        result.left += rhs.left;
        result
    }
}
impl Spacing {
    pub fn from(raw: Option<String>) -> Spacing {
        let mut result: Spacing = Spacing::default();
        if let Some(value) = raw {
            result = Spacing::from_str(&value).unwrap();
        }
        result
    }
}

/// Return the horizontal padding and margin for an image.
#[inline]
pub fn horizontal_spacing(
    padding_left: u32,
    padding_right: u32,
    margin_left: u32,
    margin_right: u32,
) -> u32 {
    padding_left + padding_right + margin_left + margin_right
}

/// Return the vertical padding and margin for an image.
#[inline]
pub fn vertical_spacing(
    padding_top: u32,
    padding_bottom: u32,
    margin_top: u32,
    margin_bottom: u32,
) -> u32 {
    padding_top + padding_bottom + margin_top + margin_bottom
}

/// Return the total length (width or height) of the image taking count of the margin, padding and ratio.
pub fn absolute_length(length: u32, horizontal_spacing: u32, ratio: f32) -> u32 {
    round_up(length.to_f32().unwrap() + horizontal_spacing.to_f32().unwrap() * ratio)
}

#[inline]
pub fn round_up(value: f32) -> u32 {
    value.round().max(0f32) as u32
}

/// Return the nearest fraction. If fraction. Fraction is not available, return a fraction.
///
/// *Note: used for Opera CSS pixel-ratio media queries.*
pub fn nearest_fration(value: f32) -> String {
    format!("{}", fraction::Fraction::from(value))
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
        assert_eq!(
            Spacing::from(Some("5".to_string())),
            Spacing {
                top: 5,
                right: 5,
                bottom: 5,
                left: 5
            }
        );
        assert_eq!(
            Spacing::from(Some("5 8".to_string())),
            Spacing {
                top: 5,
                right: 8,
                bottom: 5,
                left: 8
            }
        );
        assert_eq!(
            Spacing::from(Some("5 8,2".to_string())),
            Spacing {
                top: 5,
                right: 8,
                bottom: 2,
                left: 8
            }
        );
        assert_eq!(
            Spacing::from(Some("1 2,3 4".to_string())),
            Spacing {
                top: 1,
                right: 2,
                bottom: 3,
                left: 4
            }
        );
        assert_eq!(Spacing::from(None), Spacing::default());

        assert_eq!(horizontal_spacing(5, 5, 2, 2), 14);
        assert_eq!(vertical_spacing(1, 1, 2, 2), 6);
    }

    #[test]
    fn test_spacing_add() {
        let a = Spacing {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4,
        };
        let b = Spacing {
            top: 10,
            right: 20,
            bottom: 30,
            left: 40,
        };
        let result = a + b;
        assert_eq!(
            result,
            Spacing {
                top: 11,
                right: 22,
                bottom: 33,
                left: 44
            }
        );

        // Test with zero
        let zero = Spacing::default();
        assert_eq!(a + zero, a);
    }

    #[test]
    fn test_absolute_length() {
        // No spacing, ratio 1.0
        assert_eq!(absolute_length(100, 0, 1.0), 100);

        // With spacing, ratio 1.0
        assert_eq!(absolute_length(100, 10, 1.0), 110);

        // No spacing, ratio 2.0
        assert_eq!(absolute_length(100, 0, 2.0), 100);

        // With spacing, ratio 2.0 (spacing is multiplied by ratio)
        assert_eq!(absolute_length(100, 10, 2.0), 120);

        // With spacing, ratio 0.5
        assert_eq!(absolute_length(100, 10, 0.5), 105);
    }

    #[test]
    fn test_spacing_from_str() {
        use std::str::FromStr;

        // Single value
        let s = Spacing::from_str("10").unwrap();
        assert_eq!(s, Spacing { top: 10, right: 10, bottom: 10, left: 10 });

        // Two values (vertical, horizontal)
        let s = Spacing::from_str("5 10").unwrap();
        assert_eq!(s, Spacing { top: 5, right: 10, bottom: 5, left: 10 });

        // Three values (top, horizontal, bottom)
        let s = Spacing::from_str("1 2 3").unwrap();
        assert_eq!(s, Spacing { top: 1, right: 2, bottom: 3, left: 2 });

        // Four values (top, right, bottom, left)
        let s = Spacing::from_str("1 2 3 4").unwrap();
        assert_eq!(s, Spacing { top: 1, right: 2, bottom: 3, left: 4 });
    }
}
