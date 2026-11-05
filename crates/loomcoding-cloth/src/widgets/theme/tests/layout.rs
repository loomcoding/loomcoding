//! Tests: disposición (layout) como preferencia independiente del color.

use super::*;

#[test]
fn layout_names_round_trip() {
    let all = [
        LayoutStyle::Workspace,
        LayoutStyle::ContextRail,
        LayoutStyle::Minimal,
        LayoutStyle::Cards,
    ];
    for layout in all {
        assert_eq!(LayoutStyle::from_name(layout.name()), Some(layout));
    }
    assert_eq!(LayoutStyle::Workspace.name(), "workspace");
    assert_eq!(LayoutStyle::ContextRail.name(), "rail");
    assert_eq!(LayoutStyle::Minimal.name(), "minimal");
    assert_eq!(LayoutStyle::Cards.name(), "cards");
    assert_eq!(LayoutStyle::from_name("desconocido"), None);
    assert_eq!(LayoutStyle::from_name(""), None);
}

#[test]
fn next_cycles_rail_minimal_cards() {
    assert_eq!(LayoutStyle::ContextRail.next(), LayoutStyle::Minimal);
    assert_eq!(LayoutStyle::Minimal.next(), LayoutStyle::Cards);
    assert_eq!(LayoutStyle::Cards.next(), LayoutStyle::ContextRail);
    assert_eq!(LayoutStyle::Workspace.next(), LayoutStyle::ContextRail);
}
