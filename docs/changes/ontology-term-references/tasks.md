## 1. Implementation

- [x] 1.1 `terms.rs`: read labeled classes, properties, and SKOS concepts from Turtle, the glossary fallback, and tags with the `-KEY` rule
- [x] 1.2 `lint.rs`: term lookup through the source, `term-reference-missing` and `term-reference-unresolved`, and `lint_document` for records and runes
- [x] 1.3 `rune spec glossary` with `--json`, and the `deck.yaml` `ontology` key through the lookup hook
- [x] 1.4 `rune validate`: term check over `docs/decisions` and `runes` (deck) or the module content directories, when an ontology exists

## 2. Verification

- [x] 2.1 Lint unit tests: ontology replaces glossary, cited first use passes, missing tag, wrong IRI, undefined tag, namespace definition of a non-term, document code without capability
- [x] 2.2 Integration tests: `rune spec glossary` text and JSON, `rune validate` fails an uncited rule and passes a cited one, skips without an ontology
- [x] 2.3 `cargo test --all-features`, clippy, fmt, `rune spec validate --source .`, the deck validates clean against its own ontology
- [x] 2.4 Adversarial review by astra (25 hits, workshop `docs/specs/2026-09-23-ontology-terms/`), 18 fixes and 7 rejections recorded

## 3. Documentation

- [x] 3.1 Spec walkthrough house rules, cli CHANGELOG, rune-docs CHANGELOG, ADR CLI-0043
