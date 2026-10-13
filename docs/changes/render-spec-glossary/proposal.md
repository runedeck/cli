---
status: proposed
---

# Render Spec Glossary

## Why

A reader of a specification meets a term in italics and wants its definition one hop away. That hop exists only when the repository carries a human-readable glossary. The ontology, `ontology/rune.ttl`, is the single home of every term, and it is Turtle, which a reader of Markdown does not open. `rune spec glossary` prints the term list to a terminal, so the list lives in no file a reader or a link can reach. The deck keeps a Python script that writes and checks `docs/specs/glossary.md` for a rune without the command, and that script is a second renderer of the same bytes.

OpenSpec has no glossary artifact or command: the `dist/` tree of openspec 1.12.0 holds no match for the word, and its README links only the project's own documentation page. The CLI owns the file.

## What Changes

- `rune spec glossary --write [PATH]` writes the glossary file from the ontology: a header, one entry per labeled term, and the reference definitions. PATH defaults to `glossary.md` beside the capability specifications.
- `rune spec glossary --check [PATH]` renders the same text in memory, compares bytes with the file, prints the terms that differ, and exits 1 on drift or a missing file.
- The bare command and `--json` keep printing the term list.

## Capabilities

- render-spec-glossary (new)

## Impact

- `rune-docs/src/spec/terms.rs`, `src/cli/spec.rs`, `src/cli/mod.rs`, `tests/spec_terms.rs`.
- `docs/walkthroughs/Spec.md`, the two changelogs.
- The deck replaces `scripts/fallback/generate-glossary` with the two flags and adds the check to CI.
