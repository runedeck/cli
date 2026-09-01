## 1. Implementation

- [x] 1.1 Record the decision in CLI-0031
- [x] 1.2 Add the theme module with named palettes, attribution, and resolution
- [x] 1.3 Add the `theme` config keys and extend the config reference
- [x] 1.4 Route `Sheet` tones through the resolved palette
- [ ] 1.5 Derive the TUI palette from the resolved palette
- [x] 1.6 Add best-effort appearance detection with the configured fallback

## 2. Verification

- [x] 2.1 Test resolution precedence: `--no-color`, `NO_COLOR`, non-terminal, theme
- [x] 2.2 Test named selection, unknown-name warning, and token overrides
- [ ] 2.3 Pin golden TUI snapshots to one named theme
- [x] 2.4 Run formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and the tests
