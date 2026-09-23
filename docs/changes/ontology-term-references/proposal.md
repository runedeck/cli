---
adr: docs/changes/ontology-term-references/adr.md
status: proposed
decisions: ["Ontology as the term source"]
---

# Ontology Term References

## Why

CLI-0040 made an italic term an error unless `docs/specs/glossary.md` defines it. The deck now defines every term once, in `ontology/rune.ttl`, generated the glossary from it with a script, and checked the citations with a second script. Two scripts and a generated file carried what one validator should: the term exists, and the first use of it names the IRI. This change moves both checks into `rune spec validate` and `rune validate`, and adds `rune spec glossary` for the human-readable list, so the deck deletes the scripts and the generated file.

## What Changes

- The term source is the ontology when the repository has one, else the glossary. `spec-term-undefined` and `delta-term-undefined` keep their names.
- The first italic use of an ontology term in a file must cite it (`term-reference-missing`), and the tag must resolve to the term's IRI (`term-reference-unresolved`). rumdl MD052 and MD053 keep reporting undefined and unused tags.
- `rune spec glossary` prints the term list with tags and definitions.
- `rune validate` applies the reference rules to `docs/decisions` and `runes` of a deck, and to the record and rune directories of a module, when an ontology exists. Plain emphasis there is not a term.
- The `Defined terms have a glossary entry` requirement of `spec-house-rules` is superseded by `Defined terms resolve to the ontology` once both changes archive.

## Scope

- `rune-docs/src/spec/terms.rs` (new), `lint.rs`, `validate.rs`, and `mod.rs`, plus `src/cli/spec.rs`, `src/cli/validate/mod.rs`, and `src/cli/mod.rs`.
- Not in scope: reading `context.jsonld` for the graph export (cli #70), or any change to the ontology itself.
