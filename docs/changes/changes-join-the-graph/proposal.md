---
adr: "docs/changes/changes-join-the-graph/adr.md"
status: proposed
decisions: ["The graph export reads the deck's context and emits the lifecycle"]
---

# Graph export lifecycle

## Why

A deck checks its artifact graph with SHACL shapes, and a shape can only test what the data graph holds ([SHACL][SHACL]). The deck's shapes test a record's `status` and `supersedes`. Its inference rules need changes, capabilities, requirements, and scenarios as nodes, so that a record's `upstream` reference resolves inside the graph. An exporter with its own field list needs a cli release for each new frontmatter key. JSON-LD 1.1 defines a *context* as the document that maps a term to an IRI and to a value type ([JSON-LD 1.1, section 3.1][JSONLD]). The deck keeps one at `ontology/context.jsonld`, so the exporter reads the meaning of each key from the deck (runedeck/cli#70, #71).

## What Changes

- The exporter loads `<deck>/ontology/context.jsonld` and emits one triple per frontmatter key the context maps, in the form the term states: an IRI for `@id`, a typed date for `xsd:date`, a `rune:` term for `@vocab`, a string otherwise, and every item of a list for `@set`. The exporter drops an unmapped key. Without a context file, a record carries its identifier, title, relations, and source path.
- Changes, capabilities, requirements, scenarios, and *draft records* join the graph. A draft record is the `adr.md` of an active change, a decision record that has no identifier yet. A change is `/id/change/<id>`, a requirement is `<capability>#<slug>`, and a scenario is `<capability>#<requirement-slug>/<scenario-slug>`, so a record's `upstream` reference resolves to the node the spec produces.
- The namespace is `https://runedeck.ai/` (DECK-0010).

## Capabilities

- changes-join-the-graph (new)

## Impact

- `src/cli/graph/mod.rs`, `src/cli/graph/context.rs`, `src/cli/graph/lifecycle.rs`, `tests/graph_export.rs`, `src/cli/deploy/tests.rs` (namespace only).
- The deck's `ontology/shapes.ttl` status shapes can move from Warning to Violation once a release carries this exporter.

[SHACL]: https://www.w3.org/TR/shacl/
[JSONLD]: https://www.w3.org/TR/json-ld11/#the-context
