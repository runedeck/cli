---
type: proof
change: install-deck-ontology
head: e60aed9434ed4149e663f682e70a3f70a87a12dc
recorded: 2026-10-16
transcript: f92e040bc56084a5b92e913caf49cbc1963873d3d26a1431e44e82982d33fccb
scenes:
- scenario: install-deck-ontology#consumer-names-its-ontology-source/key-names-an-absent-source
  kind: check
- scenario: install-deck-ontology#install-deploys-the-ontology-with-provenance/consumer-cites-a-deck-term
  kind: check
- scenario: install-deck-ontology#consumer-without-the-key-installs-as-before/manifest-carries-no-ontology-key
  kind: check
- scenario: install-deck-ontology#source-without-an-ontology-file-is-refused/source-carries-no-ontology-file
  kind: check
- scenario: install-deck-ontology#deployed-copy-reports-drift/copy-edited-by-hand
  kind: check
---

# Proof: install-deck-ontology

One scene per scenario, in closure order. Each scene copies the fixture deck from `tests/fixtures/install-ontology/deck`, one rule beside an ontology with three terms, next to a consumer whose `.rune` names it. Fill each `console` fence with `$ command`, an optional `? <status>`, and the expected output, where `[..]` and `...` elide. Then run `rune proof run install-deck-ontology`.

## install-deck-ontology#consumer-names-its-ontology-source/key-names-an-absent-source

```console
$ sh -s
< cp -R "$RUNE_PROOF_ROOT/tests/fixtures/install-ontology/deck" deck
< mkdir consumer
< printf 'version: 1\nsources:\n  deck:\n    local: ../deck\nontology: elsewhere\n' > consumer/.rune
$ rune install --source consumer
? 2
fatal: .rune: ontology 'elsewhere' has no matching `sources` entry
$ sh -c 'test ! -e consumer/ontology && echo "no ontology copy"'
no ontology copy
```

## install-deck-ontology#install-deploys-the-ontology-with-provenance/consumer-cites-a-deck-term

```console
$ sh -s
< cp -R "$RUNE_PROOF_ROOT/tests/fixtures/install-ontology/deck" deck
< mkdir -p consumer/docs/changes/cite-the-instance/specs/deployed-copy-records
< printf 'version: 1\nsources:\n  deck:\n    local: ../deck\nrunes:\n  deck:\n    rules: [CiteTerms]\nontology: deck\n' > consumer/.rune
< printf -- '---\nstatus: proposed\n---\n\n# Cite the instance\n\n## Why\n\nA reader meets a term.\n\n## What Changes\n\n- Cite it.\n\n## Capabilities\n\n- deployed-copy-records (new)\n\n## Impact\n\n- The specification.\n' > consumer/docs/changes/cite-the-instance/proposal.md
< printf '## 1. Work\n\n- [ ] 1.1 Cite\n' > consumer/docs/changes/cite-the-instance/tasks.md
< printf '## ADDED Requirements\n\n### Requirement: Copy carries a record\n\nEvery *[instance][INSTANCE]* MUST carry a record.\n\n#### Scenario: Copy lands\n\n- **WHEN** a copy lands\n- **THEN** its record exists\n\n[INSTANCE]: ../../../../specs/glossary/instance.md\n' > consumer/docs/changes/cite-the-instance/specs/deployed-copy-records/spec.md
$ rune spec validate cite-the-instance --source consumer
specification validation passed
$ rune install --source consumer

 ✓ claude
   rules 1
   ● CiteTerms
 ✓ ontology
   files 1
   ● rune.ttl

 ● 2 deployed
$ cmp deck/ontology/rune.ttl consumer/ontology/rune.ttl
$ cat consumer/ontology/.manifest
rune.ttl:
  fingerprint: [..]
  provenance: .provenance/rune.ttl.yaml
$ sh -c 'grep -E "name: ontology/rune.ttl|source: |uri: " consumer/ontology/.provenance/rune.ttl.yaml'
  - name: ontology/rune.ttl
        source: [..]/deck
      - uri: ontology/rune.ttl
$ rune spec validate cite-the-instance --source consumer
? 1
error[delta-term-page]: docs/changes/cite-the-instance/specs/deployed-copy-records/spec.md:5: glossary page docs/specs/glossary/instance.md is missing; run `rune spec glossary --write`
$ rune spec glossary --write --source consumer
wrote docs/specs/glossary: 3 page(s) and the index, 4 file(s) changed
$ rune spec validate cite-the-instance --source consumer
specification validation passed
```

## install-deck-ontology#consumer-without-the-key-installs-as-before/manifest-carries-no-ontology-key

```console
$ sh -s
< cp -R "$RUNE_PROOF_ROOT/tests/fixtures/install-ontology/deck" deck
< mkdir consumer
< printf 'version: 1\nsources:\n  deck:\n    local: ../deck\nrunes:\n  deck:\n    rules: [CiteTerms]\n' > consumer/.rune
$ rune install --source consumer

 ✓ claude
   rules 1
   ● CiteTerms

 ● 1 deployed
$ sh -c 'test -f consumer/.claude/rules/CiteTerms.md && test ! -e consumer/ontology && echo "rule deployed, no ontology copy"'
rule deployed, no ontology copy
```

## install-deck-ontology#source-without-an-ontology-file-is-refused/source-carries-no-ontology-file

```console
$ sh -s
< cp -R "$RUNE_PROOF_ROOT/tests/fixtures/install-ontology/deck" deck
< rm deck/ontology/rune.ttl
< mkdir consumer
< printf 'version: 1\nsources:\n  deck:\n    local: ../deck\nrunes:\n  deck:\n    rules: [CiteTerms]\nontology: deck\n' > consumer/.rune
$ rune install --source consumer
? 2
fatal: .rune: ontology 'deck' names source 'deck' at [..]/deck, which has no ontology file at ontology/rune.ttl
$ sh -c 'test ! -e consumer/.claude && test ! -e consumer/ontology && echo "no provider tree, no ontology copy"'
no provider tree, no ontology copy
```

## install-deck-ontology#deployed-copy-reports-drift/copy-edited-by-hand

```console
$ sh -s
< cp -R "$RUNE_PROOF_ROOT/tests/fixtures/install-ontology/deck" deck
< mkdir consumer
< printf 'version: 1\nsources:\n  deck:\n    local: ../deck\nrunes:\n  deck:\n    rules: [CiteTerms]\nontology: deck\n' > consumer/.rune
$ rune install --source consumer

 ✓ claude
   rules 1
   ● CiteTerms
 ✓ ontology
   files 1
   ● rune.ttl

 ● 2 deployed
$ sh -c 'printf "# edited by hand\n" > consumer/ontology/rune.ttl'
$ sh -c 'cd consumer && rune drift'
? 1

 ontology
   ┌ rune.ttl
   │  frontmatter  ✓
   │  body         ⚡ drifted
   └
   1 identical hidden — --all shows them

 ✓ 1 identical  ⚡ 1 drifted
$ rune drift --source consumer --target consumer
? 1

 ontology
   ┌ ontology/rune.ttl (source: [..]/deck)
   │  frontmatter  ✓
   │  body         ⚡ drifted
   └
   1 identical hidden — --all shows them

 ✓ 1 identical  ⚡ 1 drifted
$ sh -c 'rune status --source consumer | grep ontology'
   ontology  0 ok · 1 stale  consumer/ontology
$ rune install --source consumer

 ✓ claude
 ✓ ontology
   ○ consumer/ontology/rune.ttl — user modified

 ○ 2 skipped
$ cat consumer/ontology/rune.ttl
# edited by hand
$ rune install --source consumer --force

 ✓ claude
 ✓ ontology
   files 1
   ● rune.ttl

 ● 1 deployed  ○ 1 skipped
$ cmp deck/ontology/rune.ttl consumer/ontology/rune.ttl
$ sh -c 'cd consumer && rune drift'

   2 identical hidden — --all shows them

 ✓ 2 identical
```
