---
title: "Ontology as the term source"
description: "rune spec validate and rune validate read defined terms from ontology/rune.ttl, require the first italic use of a term to cite its IRI, and print the term list with rune spec glossary; glossary.md stays as the fallback for a repository without an ontology"
type: adr
category: architecture
tags:
    - cli
    - spec
    - ontology
status: proposed
created: 2026-09-23
updated: 2026-09-23
author: "@N4M3Z"
project: cli
related:
    - "CLI-0040 Specification House Rules"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude", "gpt-6-astra@codex"]
informed: []
upstream: []
change: ontology-term-references
---

# Ontology as the term source

## Context and Problem Statement

CLI-0040 made an italic term an error unless `docs/specs/glossary.md` carries a `- **term**: definition` bullet. The deck then defined every term once, in `ontology/rune.ttl` with `rdfs:label` and `rdfs:comment`, and wrote two scripts: one generated the glossary from the ontology so the validator would accept the terms, the other checked that the first italic use of a term cites its IRI as `*term* [TAG]` with `[TAG]: https://runedeck.ai/ns#Term`. The generated file drifted whenever the ontology changed without a regeneration, the citation check ran only where someone invoked the script, and neither check reached decision records or runes, where the same terms appear. rumdl already reports an undefined `[TAG]` (MD052) and an unused definition (MD053), so those two checks need no second home.

## Considered Options

1. **Keep the scripts and the generated glossary**, and add a CI step per repository. Every repository copies two scripts and a regeneration hook, and the citation rule stays outside `rune spec validate`.
2. **Read the ontology in `rune spec validate`**, cite at first use, print the list with `rune spec glossary`, and run the same rules from `rune validate` over records and runes. `glossary.md` stays as the source for a repository without an ontology.
3. **Parse the ontology with an RDF library.** Correct for every Turtle form, and a dependency the crate does not otherwise carry, for a file whose shape the deck controls.

## Decision Outcome

Option 2.

`rune-docs/src/spec/terms.rs` reads every labeled class, property, and SKOS concept from the ontology with two regular expressions over the Turtle statement shape the deck writes, and falls back to the glossary. The lint resolves an italic term against that source with the existing case-insensitive and plain-plural rule, so `spec-term-undefined` and `delta-term-undefined` keep their names. With an ontology, the first italic use of each term in a file must carry `[TAG]` and the file must define the tag to the term's IRI: `term-reference-missing` and `term-reference-unresolved`. A definition under the ontology namespace that names no term is also unresolved. `rune spec glossary` prints the list the generator used to write, tags in upper case with a `-KEY` suffix where a property collides with a class. `rune validate` applies the term rules to `docs/decisions` and `runes` of a deck root and to the record and rune directories of a module root, when the root has an ontology, and reports nothing otherwise. Outside the specification tree only the reference rules run: an italic run that matches no term is emphasis there (`*not*`, an attribution line), not a definition, and YAML front matter is skipped everywhere. `deck.yaml` may name the ontology under `ontology`, and the crate receives the path through a lookup hook like `spec.root`.

### Consequences

- [+] One definition per term, in the ontology, and one validator for its use, in `rune`. The deck deletes `scripts/generate-glossary`, `scripts/check-terms`, and `docs/specs/glossary.md`.
- [+] Records and runes get the same check as specifications, from the command that already validates them.
- [-] The Turtle reader is a statement-shape parser, not an RDF parser. It reads `prefix:Name a <type> ... .` with the period ending a line, `rdf:type` for `a`, comment lines, and the `\"` and `\\` escapes. A term written another way (a full-IRI subject, a type list, a triple-quoted literal) is invisible until someone writes it the deck's way. What the reader can detect it refuses: an undeclared prefix, two terms of one kind whose labels match without case, and an ontology with no labeled term are errors, and only a missing file falls back to the glossary. A class and a property may share a label (`Change` and `change` in the deck). The italic term is the class.
- [-] The Markdown side is a line scanner, not a parser: an italic run is `*text*` with no `*` or word character on either side and no backslash before it, code is a backtick or tilde fence or a backtick span, and a front matter block is skipped when `---` opens the file and `---` or `...` closes it. Indented code blocks and HTML comments are scanned as prose.
- [-] Every first use of a term now needs a tag and a definition line, so a file with many terms grows a reference block at its end. `rune spec glossary` prints the lines to paste.
- [-] The `Defined terms have a glossary entry` requirement of `spec-house-rules` is superseded by this change's requirement once both archive. Until then the two deltas describe one rule twice.
