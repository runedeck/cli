## 1. Implementation

- [x] 1.1 Record the decision in CLI-0036
- [x] 1.2 Add the `light` flag to the theme tones and derive the TUI palette from the theme
- [x] 1.3 Route every TUI color through the palette accessors
- [x] 1.4 Show deck, target, and provider states in the status bar
- [x] 1.5 Render the first-run panel and footer hint when no deck and no modules exist
- [x] 1.6 Name the close keys and version in the help overlay

## 2. Verification

- [x] 2.1 Test the light and dark palette derivation and the tone mapping
- [x] 2.2 Test the first-run panel, the status bar context, and the help title
- [x] 2.3 Run formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and the tests
