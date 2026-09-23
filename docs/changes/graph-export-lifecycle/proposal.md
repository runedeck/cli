---
adr: "docs/changes/graph-export-lifecycle/adr.md"
status: proposed
decisions: ["The graph export reads the deck's context and emits the lifecycle"]
---

# Graph export lifecycle

## Why

`rune graph export` emits decision records and rules with `dcterms:relation` and nothing else, and every predicate is a constant in Rust. The deck's shapes now check `status` and `supersedes`, its inference register needs changes, capabilities, requirements, and scenarios as nodes, and its `ontology/context.jsonld` already says what each frontmatter key means. The exporter has to read the context and the lifecycle instead of a fixed field list (runedeck/cli#70, #71).

## What Changes

- The exporter loads `<deck>/ontology/context.jsonld` and emits one triple per frontmatter key the context maps, in the form the term states: an IRI for `@id`, a typed date for `xsd:date`, a `rune:` term for `@vocab`, a string otherwise, and every item of a list for `@set`. An unmapped key is dropped. A missing context file leaves the output as it was.
- Changes, capabilities, requirements, scenarios, and draft records join the graph. A change is `/id/change/<id>`, a requirement is `<capability>#<slug>` and a scenario `<capability>#<requirement-slug>/<scenario-slug>`, so a record's `upstream` reference resolves to the node the spec produces.
- The namespace is `https://runedeck.ai/` (DECK-0010 amendment of 2026-09-23).

## Capabilities

- graph-export-lifecycle (new)

## Impact

- `src/cli/graph/mod.rs`, `src/cli/graph/context.rs`, `src/cli/graph/lifecycle.rs`, `tests/graph_export.rs`, `src/cli/deploy/tests.rs` (namespace only).
- The deck's `ontology/shapes.ttl` status shapes can move from Warning to Violation once this is released.
