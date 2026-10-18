---
status: proposed
---

# Install deck ontology

## Why

There is one ontology, the deck's `ontology/rune.ttl`, and every specification that links a term links one it defines. `rune spec validate` and `rune spec doctor` read terms from `ontology/rune.ttl` at the repository root, or from the path the `ontology` key of `deck.yaml` names. A consumer carries no `deck.yaml`, so it has no way to read the deck's ontology today, and a term link in its specifications goes unchecked. Once the copy exists, the link fails with the page diagnostic until the glossary pages are written. The cli repository is the first consumer: its specifications link *[instance][INSTANCE]*, *[provenance][PROVENANCE]*, *[sidecar][SIDECAR]*, and *[target][TARGET]*, which only that file defines.

The deck already deploys runes into a consumer with a fingerprint and a [provenance][PROVENANCE] [sidecar][SIDECAR] per file. The ontology is one more file from the same source, and it gets the same record.

## What Changes

- `.rune` gains a top-level key `ontology: <source-name>`, naming one of its `sources`. One ontology per consumer. A name that matches no source fails the parse with a message that names the key and the name.
- `rune install` in consumer mode copies that source's ontology, `<source>/ontology/rune.ttl` or the path its `deck.yaml` names under `ontology`, to `<consumer>/ontology/rune.ttl`, byte for byte. The copy gets `ontology/.provenance/rune.ttl.yaml` and a fingerprint entry in `ontology/.manifest`, through the manifest and [sidecar][SIDECAR] code the provider trees use. A hand-edited copy is skipped as user modified unless `--force` is passed. The source is resolved before any provider tree is written, so a source with no ontology file fails the install with a message that names the key and the source and writes nothing.
- `rune drift` with no mode verifies `ontology/.manifest` beside the provider manifests. `rune drift --target` compares the source ontology with the deployed copy. `rune status` lists `ontology` as a deploy [target][TARGET] with its ok and stale counts.
- Nothing changes in the term reader: the copy sits at the default path, so `rune spec validate` in the consumer finds it.
- A consumer without the key installs as before. Removing the key leaves the copy in place, because prune does not reach it.

## Capabilities

- ontology-deploys-with-provenance (new)

## Impact

- `src/cli/dotrune/parse.rs`, `src/cli/deploy/ontology.rs`, `src/cli/deploy/mod.rs`, `src/cli/drift/mod.rs`, `src/cli/drift/scope.rs`, `src/cli/status.rs`, `src/deck/mod.rs`, `src/cli/spec.rs`.
- `tests/install_ontology.rs`, `tests/fixtures/install-ontology/`.
- `docs/walkthroughs/Spec.md`, the two changelogs.
- The cli repository becomes a consumer of itself: `.rune` names the deck as a git source and `ontology: deck`, and the deployed copy is committed with its [sidecar][SIDECAR] once the deck's ontology is on origin main.

[INSTANCE]: ../../specs/glossary/instance.md
[PROVENANCE]: ../../specs/glossary/provenance.md
[SIDECAR]: ../../specs/glossary/sidecar.md
[TARGET]: ../../specs/glossary/target.md
