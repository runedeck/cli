## 1. Implementation

- [ ] 1.1 Record the decision in CLI-0032
- [x] 1.2 Add the `.rune` provider overlays with the schema version step
- [x] 1.3 Add the `on` and `off` verbs to the kind commands
- [x] 1.4 Add the per-provider state matrix to `rune <kind> list`
- [x] 1.5 Apply the overlays in assemble and prune toggled-off deployments in install
- [ ] 1.6 Add the TUI matrix editor over the overlay data

## 2. Verification

- [x] 2.1 Test overlay resolution: base, exclude, include override
- [x] 2.2 Test the toggle verbs, the all-provider default, and ambiguous names
- [x] 2.3 Test byte-exact preservation of unrelated `.rune` content
- [x] 2.4 Test install pruning of toggled-off deployments
- [x] 2.5 Run formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and the tests
