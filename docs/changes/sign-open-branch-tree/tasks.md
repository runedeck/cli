# Tasks

## 1. Implementation

- [x] 1.1 `Repo::file_at`, `read_body` and `validate_body` from the head tree, owner-signed head accepted in `qualify`
- [x] 1.2 `purpose_from_proposal` and `CanonicalSpec::new` with a purpose
- [x] 1.3 Doctor reports parse issues with their line
- [x] 1.4 Promote compares provider copies
- [x] 1.5 `[workspace] members = ["rune-docs"]`, `--workspace` in `test.yaml`, `release.yaml`, and the `cargo-test` hook

## 2. Verification

- [x] 2.1 `tests/sign_queue.rs`: owner-signed head sealed above, body read from the branch not the working copy
- [x] 2.2 Unit tests for the purpose lookup, the doctor finding, and the divergent promote
- [x] 2.3 `cargo test --workspace --all-features`, clippy, and rustfmt clean in the mirror
- [x] 2.4 Both prek stages in an isolated clone

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
