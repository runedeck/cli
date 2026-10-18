---
adr: docs/changes/every-term-cites-definition/adr.md
status: proposed
decisions: ["Ontology as the term source"]
---

# Ontology Term References

## Why

CLI-0040 made an italic term an error unless `docs/specs/glossary.md` defines it. The deck then defined every term once, in `ontology/rune.ttl`, generated the glossary from it with a script, and checked the citations with a second script. Two scripts and a generated file carried what belongs in one validator. Italics also marked a term with a form no reader could tell from emphasis, so a term carried meaning only the checker saw. This change moves the checks into `rune spec validate` and `rune validate`, marks a term as a link to its generated glossary page, adds `rune spec fix` for the repairs a validator must not make, and has archive refuse a change whose terms are not settled.

## What Changes

- A term is a Markdown link to its glossary page: emphasized at the first use in a file, plain after it, with a reference definition at the end of the file that points at the page by relative path. Plain italics are emphasis and are not checked.
- The term source is the ontology. A repository without one has no terms, so the glossary fallback goes away. A term may carry a draft status.
- The validator reports a link that names no term, a draft term, a first use that is not emphasized or a later use that is, a page that is missing or stale, and the legacy italics form, which still resolves and warns with the new form to write. The design's Commands table names the diagnostic ids. rumdl MD052 and MD053 keep reporting an undefined and an unused tag.
- `rune spec glossary` prints the term list with tags and page definitions.
- `rune spec fix --terms` adds a draft entry and a page for every link that names no term. `rune spec fix --links` migrates the legacy form in every Markdown file. Both change nothing on a second run.
- `rune spec archive` refuses a change whose files link to a draft term or to no term.
- `rune validate` applies the term rules to `docs/decisions` and `runes` of a repository that carries a deck marker, and to the record and rune directories of a module, when an ontology exists.
- The `Defined terms have a glossary entry` requirement of `specs-must-stay-short` becomes the link rule, and this change's `Defined terms link to the ontology` supersedes it once both changes archive.

## Scope

- `rune-docs/src/spec/{terms,term_links,lint,prose,fix,glossary,commands,mod}.rs`, `src/cli/spec.rs`, `src/cli/validate/mod.rs`, and `src/cli/mod.rs`.
- Not in scope: assembly stripping the links from an instance and adding a per-instance definition block (a later change), reading `context.jsonld` for the graph export (cli #70), or any change to the ontology itself beyond the draft status.
