use std::str::FromStr;

use spriters::Ordering;

#[test]
fn test_ordering_to_display() {
    assert_eq!(Ordering::Maxside.to_string(), "maxside");
    assert_eq!(Ordering::MaxsideReverse.to_string(), "-maxside");
    assert_eq!(Ordering::Width.to_string(), "width");
    assert_eq!(Ordering::WidthReverse.to_string(), "-width");
    assert_eq!(Ordering::Height.to_string(), "height");
    assert_eq!(Ordering::HeightReverse.to_string(), "-height");
    assert_eq!(Ordering::Area.to_string(), "area");
    assert_eq!(Ordering::Area.to_string(), "-area");
    assert_eq!(Ordering::Filename.to_string(), "filename");
    assert_eq!(Ordering::Filename.to_string(), "-filename");
}

#[test]
fn test_ordering_from_str() {
    assert_eq!(Ordering::from_str("maxside").unwrap(), Ordering::Maxside);
    assert_eq!(
        Ordering::from_str("-maxside").unwrap(),
        Ordering::MaxsideReverse
    );
    assert_eq!(Ordering::from_str("width").unwrap(), Ordering::Width);
    assert_eq!(
        Ordering::from_str("-width").unwrap(),
        Ordering::WidthReverse
    );
    assert_eq!(Ordering::from_str("height").unwrap(), Ordering::Height);
    assert_eq!(
        Ordering::from_str("-height").unwrap(),
        Ordering::HeightReverse
    );
    assert_eq!(Ordering::from_str("area").unwrap(), Ordering::Area);
    assert_eq!(Ordering::from_str("-area").unwrap(), Ordering::AreaReverse);
    assert_eq!(Ordering::from_str("filename").unwrap(), Ordering::Filename);
    assert_eq!(
        Ordering::from_str("-filename").unwrap(),
        Ordering::FilenameReverse
    );
}

#[test]
fn test_ordering_is_reverse() {
    assert!(!Ordering::Maxside.is_reverse());
    assert!(Ordering::MaxsideReverse.is_reverse());
    assert!(!Ordering::Width.is_reverse());
    assert!(Ordering::WidthReverse.is_reverse());
    assert!(!Ordering::Height.is_reverse());
    assert!(Ordering::HeightReverse.is_reverse());
    assert!(!Ordering::Area.is_reverse());
    assert!(Ordering::AreaReverse.is_reverse());
    assert!(!Ordering::Filename.is_reverse());
    assert!(Ordering::FilenameReverse.is_reverse());
}
