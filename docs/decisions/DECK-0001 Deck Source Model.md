---
title: "Deck Source Model"
description: "Deck discovery, qualified artifact ids, subpath sources, cast resolution, and aggregate operations."
type: adr
category: cli
tags:
    - deck
    - sources
    - identifiers
status: accepted
created: 2026-07-13
updated: 2026-07-13
author: "Claude Fable 5 (claude-fable-5)"
project: rune
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["HarnessCouncil"]
informed: []
upstream: []
---

# Deck Source Model

## Context and Problem Statement

The CLI's source unit is a single module directory (`module.yaml` at root). The deck (runedeck/runedeck) holds many modules under `runes/<domain>/`, and target repos declare what they draw in a consumer manifest at their root. Artifact names are only unique within a domain, consumer manifests must be reproducible, and a target repo must be able to draw from the deck over git.

## Considered Options

- Keep single-module sources only and require one consumer-manifest source entry per domain
- Deck-aware discovery with a domain registry declared in deck.yaml
- Deck-aware discovery by directory scan, qualified artifact ids, and subpath source scoping

## Decision Outcome

### Identifiers

The canonical artifact id is `<domain>/<kind>/<Name>` with `<kind>` one of `skills`, `agents`, `rules`, `hooks`; the short form `<domain>/<Name>` is accepted when the name is unique within the domain. Bare names are accepted only when globally unique across the deck and are always stored fully qualified after resolution. `rune validate` at deck level fails on any cross-domain deploy-path collision (two domains shipping `skills/X`).

### Deck discovery

A source root containing `deck.yaml` is a deck. Its modules are exactly the directories `runes/*/` containing `module.yaml`, discovered by scan in lexicographic order; `deck.yaml` carries no domain registry. `deck.yaml` holds identity (`name`, `version`, `description`) and deck-level provider defaults. Configuration precedence: deck defaults, then domain `module.yaml`/`defaults.yaml`, then target-side overrides; nearest wins.

### Sources with subpaths

Consumer manifest source entries gain a `path` field naming a directory inside the materialized source. A git-pinned deck source with `path: runes/science` canonicalizes to that module; a source whose materialized root is a deck and whose `path` is absent exposes every domain. Local sources take the same field. Existing single-module sources are unchanged.

### Casts

A cast resolves to a flat set of qualified ids: `extends` unions depth-first (cycles are an error), `runes` globs include, `exclude` applies last, ordering deterministic. Casts live in the deck (`casts/*.yaml`); a consumer manifest may reference a cast by name plus local include/exclude overrides. Resolution happens at install time against the pinned deck commit, so the same manifest and pin always produce the same artifact set.

### Consumer manifest

`.rune` at the target repo root, with `.forge` read as legacy fallback. Entries reference a deck source (pinned), optionally a cast, and explicit qualified ids. `rune add <domain>[/<Name>]` rewrites `.rune` atomically (temp file, rename) and prints the install command; it does not install. `rune install` resolves, assembles, and deploys the selection, including only hooks belonging to selected domains.

### Aggregate operations

Against a deck source, `validate`, `provenance`, `drift`, and `clean` iterate all domains and emit one aggregate report; a failure in any domain fails the run. `release` accepts a domain argument and packages that module exactly as it packages a single-module source today.

### Resolution semantics

- **Collision unit**: the assembled deploy-relative path per provider (after transforms). Deck validation fails on any duplicate across domains or across manifest sources; there is no precedence.
- **Short form**: `<domain>/<Name>` must be unique across all kinds within the domain; ambiguity is a hard error listing the candidates. Bare `<Name>` must be unique across the whole deck, else the same error. Storage after resolution is always canonical (`<domain>/<kind>/<Name>`).
- **Per-entry resolution order**: cast (if referenced) → union with the entry's explicit ids → entry `exclude` last. Multiple manifest entries union; see collision rule.
- **`extends`**: parents resolve first, in listed order; pure set union; cycles are a hard error.
- **Globs** match canonical ids (`science/skills/Lit*`), `**` crosses segments, anchored to ids not paths. `exclude` removes ids regardless of whether a parent cast added them.
- **Domain identity**: the directory basename under `runes/`; `module.yaml` `name` must equal it or deck loading fails. `runes/` entries without `module.yaml` are skipped with a warning; nested `module.yaml` deeper than one level is ignored.
- **Pins**: commit SHAs, exactly as the existing git source machinery defines them; fetch or checkout failure is a hard error, never a fallback.
- **Legacy**: when both `.rune` and `.forge` exist, `.rune` wins and the run warns that `.forge` is ignored.
- **Install failure**: fail fast at the first failing domain; files already deployed in the run stay recorded in the target manifest (same contract as today's single-module install).
- **Hooks**: a domain's hooks deploy only when at least one of its artifacts is selected; hooks referencing another domain's files fail deck validation.
- **`rune add <domain>`** stores the glob `<domain>/**`; adds are idempotent; unknown ids, unknown casts, and casts referencing removed artifacts are hard errors at resolve time.
- **Deck-level config**: only the provider list for v1; nearest scope wins per key.
- **Kinds are closed for v1** (`skills`, `agents`, `rules`, `hooks`); extending them is a spec revision.
- **Output ordering**: domain lexicographic, then kind (skills, agents, rules, hooks), then name lexicographic everywhere, so drift reports stay quiet.
- **Aggregate scope**: a source with `path:` behaves as a single module for every command; a deck-root source means all domains, for `validate`, `provenance`, `drift`, and `clean` alike. `release` against a deck requires the domain argument.

## Consequences

- Single-module sources keep the current behavior, so all existing tests stay green
- Qualified ids make the consumer manifest self-documenting at the cost of longer entries
- Adding a domain is `mkdir` plus `module.yaml` with no registry edit; a malformed `module.yaml` in a new domain fails deck-wide validation immediately, which is intended
