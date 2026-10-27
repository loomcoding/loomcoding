# CodeLoom README branding

## Goal

Present CodeLoom with a consistent visual identity in the terminal UI, root
README, and documentation entry page; preserve the standalone Canvas animation
as a browser-viewable asset.

## Decisions

- The root README is English, including existing setup and architecture details.
- The opening describes CodeLoom directly; it does not present the project as
  based on DeepSeek Harness.
- A small locally stored animation and a CodeLoom mark appear near the top of
  the root README and documentation entry page.
- The animated preview follows the original Canvas movement, including cloth
  rows advancing under the shuttle; it loops on the 9.6-second pattern cycle.
- The TUI conversation header and F1 help title use the `CodeL∞M` wordmark.
- The standalone HTML animation is included as a downloadable local asset; the
  TUI help overlay does not attempt to execute HTML.
- Inspiration and research credits appear in an acknowledgements section at the
  very end, naming DeepSeek, Crush, Claude, and the 74 code editors reviewed.
- The README links local assets so the presentation works on GitHub without an
  external media host.

## Out of scope

- Website deployment, release/publishing, and changes to the existing local
  edits outside the TUI render, documentation, and new branding assets.

## Acceptance

- README prose is English and no longer attributes the opening design to
  DeepSeek Harness.
- The final section credits the named inspirations and 74 reviewed editors.
- Images and animation linked by the README exist inside this repository.
- The documentation entry point displays the animation and links the standalone
  HTML asset with clear local-opening instructions.
- The GIF preview visibly advances the woven cloth and loops at the source
  animation's repeating pattern state.
- The TUI's conversation header and F1 help overlay show the same brand mark.
