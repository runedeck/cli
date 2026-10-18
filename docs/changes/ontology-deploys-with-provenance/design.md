# Design for Install deck ontology

## Context

The [proposal](proposal.md) states the scope. The [delta specification](specs/ontology-deploys-with-provenance/spec.md) states
the behavior in role terms: the deployer copies the ontology with its record, the validator reports drift, the term
reader stays unchanged. This file maps each requirement to the command, flags, paths, and strings that implement it.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids and messages | Exit status |
|---|---|---|---|---|
| Consumer names its ontology source | The `.rune` top-level key `ontology: <source-name>` names one of its `sources`. `rune install` reads it. | none | ``.rune: ontology '<name>' has no matching `sources` entry`` | nonzero at parse, nothing written |
| Install deploys the ontology with provenance | `rune install` in consumer mode copies `<source>/ontology/rune.ttl`, or the path the source's `deck.yaml` names under `ontology`, to `<consumer>/ontology/rune.ttl`. It writes `ontology/.provenance/rune.ttl.yaml` and a fingerprint entry in `ontology/.manifest`. The term reader is `rune spec validate` and `rune spec doctor`. | none | none | 0 |
| Consumer without the key installs as before | `rune install` with no `ontology` key in `.rune`. | none | none | 0 |
| Source without an ontology file is refused | `rune install` resolves the source before any provider tree is written. | none | `install.ontology_missing`: `.rune: ontology '<name>' names source '<name>' at <path>, which has no ontology file at <relative>` | nonzero, nothing written |
| Deployed copy reports drift | `rune drift` verifies `ontology/.manifest` beside the provider manifests. `rune drift --target <dir>` compares the source ontology with the copy. `rune status` lists `ontology` as a deploy target with its ok and stale counts. `rune install` skips a hand-edited copy as user modified. `rune install --force` restores it. | none | Drift lists the copy. Status counts it stale. Install reports the copy as user modified. | not set by the requirement |
