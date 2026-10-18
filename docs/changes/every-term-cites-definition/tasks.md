## 1. Implementation

- [x] 1.1 `terms.rs`: read labeled classes, properties, and SKOS concepts from Turtle, and tags with the `-KEY` rule
- [x] 1.2 `lint.rs`: term lookup through the source, and `lint_document` for records and runes
- [x] 1.3 `rune spec glossary` with `--json`, and the `deck.yaml` `ontology` key through the lookup hook
- [x] 1.4 `rune validate`: term check over `docs/decisions` and `runes` (deck) or the module content directories, when an ontology exists

## 2. Verification

- [x] 2.1 Lint unit tests: ontology replaces glossary, cited first use passes, missing tag, wrong IRI, undefined tag, namespace definition of a non-term, document code without capability
- [x] 2.2 Integration tests: `rune spec glossary` text and JSON, `rune validate` fails an uncited rule and passes a cited one, skips without an ontology
- [x] 2.3 `cargo test --all-features`, clippy, fmt, `rune spec validate --source .`, the deck validates clean against its own ontology
- [x] 2.4 Adversarial review by astra (25 hits, workshop `docs/specs/2026-09-23-ontology-terms/`), 18 fixes and 7 rejections recorded

## 3. Documentation

- [x] 3.1 Spec walkthrough house rules, cli CHANGELOG, rune-docs CHANGELOG, ADR CLI-0043

## 4. Link model

- [x] 4.1 `term_links.rs`, `prose.rs`: the link forms, the definitions, page resolution through `iri`, the byte-aligned prose view. `lint.rs`: the five term rules, italics unchecked. The glossary fallback removed from `terms.rs`
- [x] 4.2 `fix.rs`: `rune spec fix --terms` and `--links`, no flag runs both. `commands.rs`: archive refuses a draft or undefined term link
- [x] 4.3 Unit tests per rule in `lint/tests.rs`, `tests/spec_terms.rs` for the pages, the link rules, `fix`, and the archive refusal
- [x] 4.4 The delta, `design.md` with the Commands table, this change's record, the cli CHANGELOG
- [ ] 4.5 `docs/walkthroughs/Spec.md` house rules section, `rune-docs/CHANGELOG.md`
- [ ] 4.6 Proof under `docs/proofs/every-term-cites-definition/`
- [ ] 4.7 Assembly strips the links from an instance and adds a per-instance definition block (a later change)
