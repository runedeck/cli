---
status: proposed
---

# Render Spec Glossary

## Why

A reader of a specification meets a term and wants its definition one hop away. That hop exists only when the repository carries a human-readable glossary a link can reach. The ontology, `ontology/rune.ttl`, is the single home of every term, and it is Turtle, which a reader of Markdown does not open. `rune spec glossary` prints the term list to a terminal, so the list lives in no file a reader or a link can reach. A single glossary file would need an anchor per term, which no link checker resolves and no graph tool draws as a node. The deck keeps a Python script that writes and checks the glossary for a machine without the command, and that script is a second renderer of the same bytes.

OpenSpec has no glossary artifact or command: the `dist/` tree of openspec 1.12.0 holds no match for the word, and its README links only the project's own documentation page. The CLI owns the files.

## What Changes

- `rune spec glossary --write [PATH]` writes one page per labeled term and an index into the glossary directory from the ontology. A page carries the label, the address, the kind, and the status as front matter, then the label as a heading, the comment, and a generated-from line. PATH defaults to `glossary/` beside the capability specifications. A write removes the page of a term the ontology no longer defines and the single `glossary.md` an older render wrote.
- `rune spec glossary --check [PATH]` renders every page and the index in memory, compares the directory byte for byte, names each page that is missing, extra, or changed, and exits 1 on drift or a missing directory.
- The bare command and `--json` keep printing the term list, with a page definition per term.
- Term links in every file point at the pages (the change `every-term-cites-definition`).

## Capabilities

- glossary-renders-from-ontology (new)

## Impact

- `rune-docs/src/spec/glossary.rs` (new), `terms.rs`, `src/cli/spec.rs`, `src/cli/mod.rs`, `tests/spec_terms.rs`.
- `docs/walkthroughs/Spec.md`, the two changelogs, and the proof under `docs/proofs/glossary-renders-from-ontology/`, which was recorded against the single file and must be recorded again.
- The deck's `scripts/fallback/generate-glossary` writes the same bytes for a machine without rune and hands over to the command when the installed rune writes a directory. The deck's `glossary-current` hook runs the check.
