---
title: "Ontology as the term source"
description: "rune spec validate and rune validate read defined terms from ontology/rune.ttl, mark a term as a link to its generated glossary page with the first use emphasized, repair drafts and the legacy form with rune spec fix, and refuse to archive a change whose terms are not settled"
type: adr
category: architecture
tags:
    - cli
    - spec
    - ontology
status: proposed
created: 2026-09-23
updated: 2026-10-18
author: "@N4M3Z"
project: cli
related:
    - "CLI-0040 Specification House Rules"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude", "gpt-6-astra@codex"]
informed: []
upstream: []
change: every-term-cites-definition
---

# Ontology as the term source

## Context and Problem Statement

CLI-0040 made an italic term an error unless `docs/specs/glossary.md` carries a `- **term**: definition` bullet. The deck then defined every term once, in `ontology/rune.ttl` with `rdfs:label` and `rdfs:comment`, and wrote two scripts: one generated the glossary from the ontology so the validator would accept the terms, the other checked that the first italic use of a term cites its term address, an IRI (internationalized resource identifier), as `*term* [TAG]` with `[TAG]: https://runedeck.ai/ns#Term`. The generated file drifted whenever the ontology changed without a regeneration, the citation check ran only where someone invoked the script, and neither check reached decision records or runes, where the same terms appear. Italics marked a term with a form no reader could tell from emphasis, and the IRI definition led a reader to raw Turtle or to nothing. rumdl already reports an undefined `[TAG]` (MD052) and an unused definition (MD053), so those two checks need no second home.

## Considered Options

1. **Keep the scripts and the generated glossary**, and add a CI step per repository. Every repository copies two scripts and a regeneration hook, and the citation rule stays outside `rune spec validate`.
2. **Read the ontology in `rune spec validate`**, mark a term as a link to its generated glossary page, print the list with `rune spec glossary`, repair with `rune spec fix`, and run the same rules from `rune validate` over records and runes.
3. **Parse the ontology with an RDF library.** Correct for every Turtle form, and a dependency the crate does not otherwise carry, for a file whose shape the deck controls.

## Decision Outcome

Option 2, with the mark changed from italics to a link.

`rune-docs/src/spec/terms.rs` reads every labeled class, property, and SKOS concept from the ontology with two regular expressions over the Turtle statement shape the deck writes. A term is marked as a Markdown link to its generated glossary page: `*[text][TAG]*` at the first use in a file, `[text][TAG]` after it, and `[TAG]: <relative page path>` at the end of the file. The lint resolves the link through the `iri` the page records, and through the tag when the page is absent, so `spec-term-undefined` and `delta-term-undefined` keep their names. The lint also reports a draft term (`spec-term-draft`), a first use that is not emphasized or a later use that is (`spec-term-first-use`), a page that is missing or differs from the render (`spec-term-page`), and the legacy `*term* [TAG]` form with a definition that names the term address (`spec-term-legacy`), which still resolves. Plain italics are not checked. A repository without an ontology has no terms, so a reference link there is an ordinary link.

`rune spec glossary` prints the list with tags in upper case, built from the label with a `-KEY` suffix where a property collides with a class, and a page definition per term. `rune spec fix --terms` adds a draft entry for every link that names no term and writes its page, and `rune spec fix --links` migrates the legacy form, as a step separate from validation, because a validator that edits is no longer a validator. `rune spec archive` refuses a change whose files link to a draft term or to no term.

`rune validate` applies the term rules to `docs/decisions` and `runes` of a repository that carries a deck marker and to the record and rune directories of a module root, when the root has an ontology, and reports nothing otherwise. YAML front matter and fenced code are skipped everywhere. `deck.yaml` may name the ontology under `ontology`, and the crate receives the path through a lookup hook like `spec.root`.

### Consequences

- [+] One definition per term, in the ontology, and one validator for its use, in `rune`. The deck's scripts become fallbacks for a machine without rune, and the single glossary file goes away.
- [+] A reader reaches the term's page from any file, and a vault draws the links as a graph.
- [+] Records and runes get the same check as specifications, from the command that already validates them.
- [-] The Turtle reader is a statement-shape parser. It reads `prefix:Name a <type> ... .` with the period ending a line, `rdf:type` for `a`, comment lines, and the `\"` and `\\` escapes. A term written another way (a full-IRI subject, a type list, a triple-quoted literal) is invisible until someone writes it the deck's way. What the reader can detect it refuses: an undeclared prefix, two terms of one kind whose labels match without case, and an ontology with no labeled term are errors. A class and a property may share a label (`Change` and `change` in the deck). The page decides which one a link names, and the property's page and tag carry a key suffix.
- [-] The Markdown side is a line scanner. A link is `[text][TAG]`, emphasized when single asterisks wrap it, code is a backtick or tilde fence or a backtick span, and a front matter block is skipped when `---` opens the file and `---` or `...` closes it. Indented code blocks and HTML comments are scanned as prose.
- [-] Every use of a term is now a link, and a file with many terms ends in a reference block. `rune spec glossary` prints the definitions to paste, and `rune spec fix --links` converts a file.
- [-] The `Defined terms have a glossary entry` requirement of `specs-must-stay-short` becomes the link rule, and this change's requirement supersedes it once both archive. Until then the two deltas describe one rule twice.
- [-] A consumer's ontology is an installed copy with a manifest fingerprint. The draft pass of the fixer would edit that copy and break the fingerprint, so a consumer runs only the links pass.
