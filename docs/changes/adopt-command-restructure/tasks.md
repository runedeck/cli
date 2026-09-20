## 1. Implementation

- [x] 1.1 One subject resolver in `subject.rs`, used by doctor, reseal, repair, and `rune provenance`
- [x] 1.2 Reseal prunes orphan reviewed sidecars to `.trash/<stamp>/` and rewrites stale names
- [x] 1.3 `rune move <from> <to>` with the whole-artifact, same-repository, open-session, pending-skill, and decision-record refusals, and the `transferredFrom` record
- [x] 1.4 `rune repair` as the single module-wide write path. Both doctors lose `--repair` and print the repair command
- [x] 1.5 `--replacement` and `--replacement-file` on adapt verdicts, the finalize proof, and the `.provenance/replacements/<sha256>` store
- [x] 1.6 `file://<canonical>` attribution for directory sources, the name pattern parity test, and the adopt-block-review and adopt-review-hardening residue
- [x] 1.7 PROV-0006 adopted as ASSEMBLY-0012. CLI-0023 and CLI-0027 related entries resolve

## 2. Verification

- [x] 2.1 Unit tests for the resolver, the read-only doctor, repair, the reseal prune, move, the replacement store, name parity, and directory attribution. Integration tests for stale names and deployment evidence in `rune provenance`
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`
- [x] 2.3 `rune spec validate`, `rune validate`, `rune docs check`, `rune adopt doctor --root .`, `make validate`

## 3. Documentation

- [x] 3.1 Walkthrough, Command Map, Exit Codes, Manual Testing, Manual Check, agent guide, skill template, changelog
