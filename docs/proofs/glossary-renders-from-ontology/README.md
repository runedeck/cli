---
type: proof
change: glossary-renders-from-ontology
head: d935b16de3e7e20588382adbe85efeebcbe84934
recorded: 2026-10-17
transcript: db58d4f185878ac5426b26b8c6bcd3ee044f6c7d896aeaa060ec3bebff6a99a0
scenes:
- scenario: glossary-renders-from-ontology#the-glossary-is-rendered-from-the-ontology/two-writes-read-one-ontology
  kind: check
- scenario: glossary-renders-from-ontology#the-check-reports-drift/page-edited-by-hand
  kind: check
- scenario: glossary-renders-from-ontology#the-check-reports-drift/glossary-directory-absent
  kind: check
---

# Proof: glossary-renders-from-ontology

One scene per scenario, in closure order. Fill each `console` fence with `$ command`, an optional `? <status>`, and the expected output, where `[..]` and `...` elide. Then run `rune proof run glossary-renders-from-ontology`.

## glossary-renders-from-ontology#the-glossary-is-rendered-from-the-ontology/two-writes-read-one-ontology

```console
$ sh -c 'mkdir -p repo/ontology && cp "$RUNE_PROOF_ROOT/tests/fixtures/spec-glossary/rune.ttl" repo/ontology/rune.ttl'
$ rune spec glossary --write --source repo
wrote docs/specs/glossary: 4 page(s) and the index, 5 file(s) changed
$ sh -c 'cp -R repo/docs/specs/glossary first'
$ rune spec glossary --write --source repo
wrote docs/specs/glossary: 4 page(s) and the index, 0 file(s) changed
$ diff -r first repo/docs/specs/glossary
$ ls repo/docs/specs/glossary
change-key.md
change.md
falsifiability.md
index.md
stage.md
$ cat repo/docs/specs/glossary/index.md
# Glossary

Generated from `ontology/rune.ttl`. Edit the ontology, not these pages.

- [change](change.md): One spec-driven unit of work.
- [change](change-key.md): The change a record belongs to.
- [falsifiability](falsifiability.md): A requirement states a check that can fail.
- [stage](stage.md):[..]
$ cat repo/docs/specs/glossary/change-key.md
---
title: change
iri: https://runedeck.ai/ns#change
kind: property
status: defined
---

# change

The change a record belongs to.

Generated from `ontology/rune.ttl`. Edit the ontology, not this page.
```

## glossary-renders-from-ontology#the-check-reports-drift/page-edited-by-hand

```console
$ sh -c 'mkdir -p repo/ontology && cp "$RUNE_PROOF_ROOT/tests/fixtures/spec-glossary/rune.ttl" repo/ontology/rune.ttl'
$ rune spec glossary --write --source repo
wrote docs/specs/glossary: 4 page(s) and the index, 5 file(s) changed
$ rune spec glossary --check --source repo
docs/specs/glossary is current
$ sh -c 'sed "s/One spec-driven unit of work./Edited by hand./" repo/docs/specs/glossary/change.md > edited.md && mv edited.md repo/docs/specs/glossary/change.md'
$ rune spec glossary --check --source repo
? 1
docs/specs/glossary is stale: run `rune spec glossary --write`
  changed  change (change.md)
```

## glossary-renders-from-ontology#the-check-reports-drift/glossary-directory-absent

```console
$ sh -c 'mkdir -p repo/ontology && cp "$RUNE_PROOF_ROOT/tests/fixtures/spec-glossary/rune.ttl" repo/ontology/rune.ttl'
$ rune spec glossary --check --source repo
? 1
docs/specs/glossary is missing: run `rune spec glossary --write`
```
