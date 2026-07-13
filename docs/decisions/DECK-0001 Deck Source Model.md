# DECK-0001 Deck Source Model

## Status

accepted

## Context

The CLI's source unit is a single module directory (`module.yaml` at root). The deck (runedeck/runedeck) holds many modules under `runes/<domain>/`, and target repos declare what they draw in a consumer manifest at their root. Artifact names are only unique within a domain, consumer manifests must be reproducible, and a target repo must be able to draw from the deck over git.

## Decision

### Identifiers

The canonical artifact id is `<domain>/<kind>/<Name>` with `<kind>` one of `skills`, `agents`, `rules`, `hooks`; the short form `<domain>/<Name>` is accepted when the name is unique within the domain. Bare names are accepted only when globally unique across the deck and are always stored fully qualified after resolution. `rune validate` at deck level fails on any cross-domain deploy-path collision (two domains shipping `skills/X`).

### Deck discovery

A source root containing `deck.yaml` is a deck. Its modules are exactly the directories `runes/*/` containing `module.yaml`, discovered by scan in lexicographic order — `deck.yaml` carries no domain registry. `deck.yaml` holds identity (`name`, `version`, `description`) and deck-level provider defaults. Configuration precedence: deck defaults, then domain `module.yaml`/`defaults.yaml`, then target-side overrides; nearest wins.

### Sources with subpaths

Consumer manifest source entries gain a `path` field naming a directory inside the materialized source. A git-pinned deck source with `path: runes/science` canonicalizes to that module; a source whose materialized root is a deck and whose `path` is absent exposes every domain. Local sources take the same field. Existing single-module sources are unchanged.

### Casts

A cast resolves to a flat set of qualified ids: `extends` unions depth-first (cycles are an error), `runes` globs include, `exclude` applies last, ordering deterministic. Casts live in the deck (`casts/*.yaml`); a consumer manifest may reference a cast by name plus local include/exclude overrides. Resolution happens at install time against the pinned deck commit, so the same manifest and pin always produce the same artifact set.

### Consumer manifest

`.rune` at the target repo root, with `.forge` read as legacy fallback. Entries reference a deck source (pinned), optionally a cast, and explicit qualified ids. `rune add <domain>[/<Name>]` rewrites `.rune` atomically (temp file, rename) and prints the install command; it does not install. `rune install` resolves, assembles, and deploys the selection, including only hooks belonging to selected domains.

### Aggregate operations

Against a deck source, `validate`, `provenance`, `drift`, and `clean` iterate all domains and emit one aggregate report; a failure in any domain fails the run. `release` accepts a domain argument and packages that module exactly as it packages a single-module source today.

## Consequences

Single-module sources keep the current behavior, so all existing tests must stay green. Qualified ids make the consumer manifest self-documenting at the cost of longer entries. Directory scan means adding a domain is `mkdir` plus `module.yaml`, with no registry edit; the price is that a malformed `module.yaml` in a new domain fails deck-wide validation immediately, which is intended.
