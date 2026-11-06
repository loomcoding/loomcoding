//! Tests: paquetes de estilo `classic`/`flat`.

use super::*;

#[test]
fn bundle_names_round_trip() {
    for bundle in StyleBundle::ALL {
        assert_eq!(StyleBundle::from_name(bundle.name()), Some(bundle));
    }
    assert_eq!(StyleBundle::Classic.name(), "classic");
    assert_eq!(StyleBundle::Flat.name(), "flat");
}

#[test]
fn bundle_from_name_rejects_unknown_text() {
    assert_eq!(StyleBundle::from_name("otro"), None);
    assert_eq!(StyleBundle::from_name(""), None);
    assert_eq!(StyleBundle::from_name("Classic"), None);
}

#[test]
fn bundle_next_rotates_and_returns_after_two_steps() {
    assert_eq!(StyleBundle::Classic.next(), StyleBundle::Flat);
    assert_eq!(StyleBundle::Flat.next(), StyleBundle::Classic);
    assert_eq!(StyleBundle::Classic.next().next(), StyleBundle::Classic);
}

#[test]
fn presets_look_matches_their_bundle() {
    assert_eq!(Look::classic().bundle(), Some(StyleBundle::Classic));
    assert_eq!(Look::crush().bundle(), Some(StyleBundle::Flat));
}

#[test]
fn with_bundle_keeps_the_layout() {
    assert_eq!(
        Look::classic().with_bundle(StyleBundle::Flat).layout,
        LayoutStyle::Workspace
    );
    assert_eq!(
        Look::crush().with_bundle(StyleBundle::Classic).layout,
        LayoutStyle::ContextRail
    );
    assert_eq!(
        Look::classic().with_bundle(StyleBundle::Flat).bundle(),
        Some(StyleBundle::Flat)
    );
}

#[test]
fn mixed_look_has_no_bundle() {
    let mixed = Look {
        composer: ComposerStyle::Crush,
        tool: ToolStyle::Pinned,
        layout: LayoutStyle::Workspace,
        chrome: ChromeStyle::Panels,
    };
    assert_eq!(mixed.bundle(), None);
}
