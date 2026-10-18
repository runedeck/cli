## 1. Implementation

- [x] 1.1 `glossary.rs`: render one page per term and the index from `GlossaryOutput`, write the directory, remove stale pages and the single file, and compare a directory against the rendering by page
- [x] 1.2 `src/cli/mod.rs` and `src/cli/spec.rs`: `--write [PATH]` and `--check [PATH]` on `rune spec glossary`, mutually exclusive, with `--json`

## 2. Verification

- [x] 2.1 Unit tests: two writes give the same bytes and remove the single file, a hand edit, a missing page, an extra file, and a missing directory are drift
- [x] 2.2 Integration tests: write then check through the binary, drift exits 1 and names the page
- [ ] 2.3 `cargo fmt --check`, clippy with `-D warnings`, the full suite, `rune spec validate glossary-renders-from-ontology`
- [x] 2.4 The command's directory over the deck's ontology, checked byte for byte against the deck's fallback script's pages (`rune spec glossary --check --source <deck>`: current)

## 3. Documentation

- [ ] 3.1 `docs/walkthroughs/Spec.md`, `CHANGELOG.md`, `rune-docs/CHANGELOG.md`
- [ ] 3.2 Proof under `docs/proofs/glossary-renders-from-ontology/`, recorded again for the directory
