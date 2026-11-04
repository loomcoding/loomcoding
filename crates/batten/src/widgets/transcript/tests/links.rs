//! Tests: links.

use super::support::*;
use super::*;

#[test]
fn links_are_wrapped_in_osc8() {
    let buf = link_buffer("ver [docs](https://x.y) ya", 40, 3, &Theme::test());
    let osc = "\x1b]8;;https://x.y\x1b\\docs\x1b]8;;\x1b\\";
    let i = buf
        .content
        .iter()
        .position(|c| c.symbol() == osc)
        .expect("celda con OSC 8");
    assert_eq!(
        buf.content[i].diff_option,
        CellDiffOption::ForcedWidth(NonZeroU16::new(4).unwrap())
    );
    assert_eq!(buf.content[i + 1].diff_option, CellDiffOption::Skip);
    assert_eq!(buf.content[i + 1].symbol(), " ");
}

#[test]
fn wrapped_links_get_one_segment_per_row() {
    let buf = link_buffer(
        "[documentacion muy larga](https://x.y)",
        20,
        4,
        &Theme::test(),
    );
    let n = buf
        .content
        .iter()
        .filter(|c| c.symbol().starts_with("\x1b]8;;https://x.y"))
        .count();
    assert_eq!(n, 2);
}

#[test]
fn hyperlinks_can_be_disabled() {
    let mut th = Theme::test();
    th.hyperlinks = false;
    let buf = link_buffer("ver [docs](https://x.y) ya", 40, 3, &th);
    assert!(buf.content.iter().all(|c| !c.symbol().contains("\x1b]8")));
}
