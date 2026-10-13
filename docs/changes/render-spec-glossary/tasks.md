## 1. Implementation

- [ ] 1.1 `terms.rs`: render the glossary file (header, entries, definitions) from `GlossaryOutput`, write it, and compare a file against the rendering by term
- [ ] 1.2 `src/cli/mod.rs` and `src/cli/spec.rs`: `--write [PATH]` and `--check [PATH]` on `rune spec glossary`, mutually exclusive, with `--json`

## 2. Verification

- [ ] 2.1 Unit tests: two writes give the same bytes, a clean check, a check after one edited term, a check with no file
- [ ] 2.2 Integration tests: write then check through the binary, drift exits 1 and names the term
- [ ] 2.3 `cargo fmt --check`, clippy with `-D warnings`, the full suite, `rune spec validate render-spec-glossary`
- [ ] 2.4 The CLI output over the deck's ontology, diffed against the deck's fallback script

## 3. Documentation

- [ ] 3.1 `docs/walkthroughs/Spec.md`, `CHANGELOG.md`, `rune-docs/CHANGELOG.md`
- [ ] 3.2 Proof under `docs/proofs/render-spec-glossary/`
