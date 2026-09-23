---
title: "The graph export reads the deck's context and emits the lifecycle"
description: "The deck's JSON-LD context says what each frontmatter key means, and the exporter emits changes, capabilities, requirements, scenarios, and drafts beside the records."
type: adr
category: cli
tags:
    - graph
    - ontology
    - export
status: proposed
created: 2026-09-23
updated: 2026-09-23
author: "@N4M3Z"
project: rune-cli
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude", "gpt-6-astra@codex"]
informed: []
upstream:
    - "https://github.com/runedeck/cli/issues/70"
    - "https://github.com/runedeck/cli/issues/71"
change: graph-export-lifecycle
---

# The graph export reads the deck's context and emits the lifecycle

## Context and Problem Statement

`rune graph export` writes a Turtle graph that the deck validates with SHACL shapes. It emits decision records and rules, and for a record only the identifier, the title, `related`, and the source path, each predicate a constant in Rust. The deck now keeps `ontology/context.jsonld`, a JSON-LD context that maps every record and proposal key to its term, and its shapes check `status` and `supersedes`. Its inference register needs changes, capabilities, requirements, and scenarios as nodes, so that a record's `upstream` reference and a scene's scenario resolve inside the graph. A frontmatter change today needs a Rust change. The question is who owns the meaning of a key: the exporter or the deck.

## Considered Options

1. Keep the field list in Rust and add the new keys by hand. Every new key is a cli release.
2. Read the deck's JSON-LD context at run time and emit what it maps. The deck owns the meaning, the exporter owns identity and paths.
3. Emit the whole frontmatter as `rune:<key>` literals and let the deck map them with SPARQL. Every key leaks, and the shapes still need the real terms.

## Decision Outcome

Option 2.

- The exporter MUST read `<deck>/ontology/context.jsonld` and MUST emit one triple per mapped key, in the object form the term states.
- The exporter MUST keep identity, title, `related`, and the source path as its own, so a deck without a context exports as before.
- An `@id` value MUST resolve by shape: record stem, `capability#slug`, URI, or a minted `/id/` IRI.
- Changes, capabilities, requirements, scenarios, and draft records MUST be nodes. A change is `/id/change/<id>`, because a single-capability change names its capability after itself. A requirement is `<capability>#<slug>`, the form a record's `upstream` already uses.
- A malformed context MUST fail the export. A missing one MUST NOT.

### Consequences

- [+] A new record key is one line in the deck's context, not a cli release.
- [+] The deck's status and supersession shapes can move from Warning to Violation, because `rune:status` is in the graph.
- [+] "Is this change proven on this commit" becomes a query over nodes that exist, once a later change adds proofs and scenes.
- [-] The exporter parses YAML frontmatter itself for lists, because the shared list helper joins items with a comma and a stem may contain one.
- [-] A canonical capability and a delta of the same name share one IRI and merge into one node with two source paths. That is the intended identity, and a shape can still see the two paths.
- [-] A draft record has no identifier, so the deck's record shape reports it until the shape allows a draft or the draft gets its id at proposal time.
- [-] The change points at the records it produced (`rune:decisionRecord`) and the record points at its change (`rune:change`). The first edge runs origin to result, against the storage rule, and stays because the deck's link check needs both sides to catch a one-sided link.
