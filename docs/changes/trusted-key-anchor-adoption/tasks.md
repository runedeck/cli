# Tasks

## 1. Implementation

- [x] 1.1 `signer_lines` and the armor fallback in `keys_fingerprints`
- [x] 1.2 `KEYS`, `scripts/trusted-keys`, `scripts/verify-seal`, `templates/skeleton/base/*`, `typos.toml` from skeleton `cc78924b`

## 2. Verification

- [x] 2.1 `signer_lines_pin_fingerprints_and_refuse_anything_else`, and `sign_queue` passes on the new `KEYS`
- [x] 2.2 `cargo test --workspace --all-features` (1495), clippy, rustfmt
- [ ] 2.3 Both prek stages in an isolated clone

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
