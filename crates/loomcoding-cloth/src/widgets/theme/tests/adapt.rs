//! Tests: adapt.

use super::*;

#[test]
fn adapt_follows_the_depth() {
    let mut th = Theme::test();
    let rgb = Color::Rgb(0, 212, 255);
    assert_eq!(th.adapt(rgb), Some(rgb));
    th.depth = ColorDepth::NoColor;
    assert_eq!(th.adapt(rgb), None);
}

#[cfg(feature = "termprofile")]
#[test]
fn adapt_lowers_rgb_with_termprofile() {
    let mut th = Theme::test();
    th.depth = ColorDepth::Ansi256;
    assert!(matches!(
        th.adapt(Color::Rgb(0, 212, 255)),
        Some(Color::Indexed(_))
    ));
    th.depth = ColorDepth::Ansi16;
    let c = th.adapt(Color::Rgb(0, 212, 255));
    assert!(c.is_some() && !matches!(c, Some(Color::Rgb(..)) | Some(Color::Indexed(_))));
}
