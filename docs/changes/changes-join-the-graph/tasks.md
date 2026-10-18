# Graph export lifecycle tasks

## 1. Implementation

- [x] 1.1 Load `ontology/context.jsonld`: prefixes, terms with `@id`, `@type` (`@id`, `xsd:date`, `@vocab`), and `@container: @set`. A malformed file is a parse error, and an absent one is today's behavior
- [x] 1.2 Emit one triple per mapped record key, keep identifier, title, relation, and source path as they were, drop unmapped keys, declare only the prefixes used
- [x] 1.3 Emit changes with `rune:archived` and `rune:decisionRecord`, delta and canonical capabilities, requirements, scenarios, and draft records
- [x] 1.4 Rename the namespace to `https://runedeck.ai/`

## 2. Verification

- [x] 2.1 `tests/graph_export.rs`: context-driven triples, lifecycle nodes, unmapped key dropped, a title in `decisions` not minted, and the no-context output unchanged
- [x] 2.2 `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` from the mirror
- [ ] 2.3 The real deck exports with zero Violations under `ontology/shapes.ttl`. Today 19 drafts fail the identifier shape and every record gets the status Warning, because the shapes expect a literal where the context maps `@vocab`. Both are deck-side (`shapes.ttl`), see the record
- [x] 2.4 Adversarial review of the diff (`/tmp/claude-501/adversary/astra-graph-export.md`): change namespace, prefix declaration without a context, context validation, vocab and URI hardening, fenced headings, empty slugs, archived drafts, the draft's origin edge. Rejected: proposal keys through the context, YAML errors as export failures, `@list`

## 3. Later

- [ ] 3.1 The deck flips its status shapes to Violation once the installed `rune` emits `rune:status`
- [ ] 3.2 Proofs and scenes (deck `prove-each-scenario` task 3.2) join the graph in a later change
