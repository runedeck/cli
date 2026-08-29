## 1. Implementation

- [ ] 1.1 Record the decision in CLI-0035
- [ ] 1.2 Add manifest discovery, parsing, and `rune plugin list`
- [ ] 1.3 Fire the bounded `post-install` event from the install path

## 2. Verification

- [ ] 2.1 Test listing, event payload, fault isolation, and executable confinement
- [ ] 2.2 Run formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and the tests
