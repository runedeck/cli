---
title: "Deck Source Model"
description: "Deck discovery, qualified rune ids, subpath sources, cast resolution, and aggregate operations."
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

The CLI's source unit is a single module directory (`module.yaml` at root). The runedeck (runedeck/runedeck) holds many decks under `runes/<deck>/`, and target repos declare what they draw in a consumer manifest at their root. A rune is a skill, an agent, or a rule; rune names are only unique within a deck, consumer manifests must be reproducible, and a target repo must be able to draw from the runedeck over git.

## Considered Options

- Keep single-module sources only and require one consumer-manifest source entry per deck
- Deck-aware discovery with a deck registry declared in deck.yaml
- Deck-aware discovery by directory scan, qualified rune ids, and subpath source scoping

## Decision Outcome

### Identifiers

The canonical rune id is `<deck>/<kind>/<Name>` with `<kind>` one of `skills`, `agents`, `rules`, `hooks` (hooks carry ids for deploy accounting but are deck infrastructure, not runes); the short form `<deck>/<Name>` is accepted when the name is unique within the deck. A bare name selects the whole deck when it names one, otherwise it must be globally unique across the runedeck. `rune validate` at runedeck level fails on any cross-deck deploy-path collision (two decks shipping `skills/X`).

### Deck discovery

A source root containing `deck.yaml` is a runedeck. Its decks are exactly the directories `runes/*/` containing `module.yaml`, discovered by scan in lexicographic order; `deck.yaml` carries no deck registry. `deck.yaml` holds identity (`name`, `version`, `description`) and runedeck-level provider defaults. Configuration precedence: runedeck defaults, then deck `module.yaml`/`defaults.yaml`, then target-side overrides; nearest wins.

### Sources with subpaths

Consumer manifest source entries take a `path` field naming a directory inside the materialized source. A git-pinned source with `path: runes/science` canonicalizes to that deck; a source whose materialized root is a runedeck and whose `path` is absent exposes every deck. Local sources take the same field. Single-module sources behave as before.

### Casts

A cast resolves to a flat set of qualified ids: `extends` unions depth-first (cycles are an error), `runes` globs include, `exclude` applies last, ordering deterministic. Casts live in the runedeck (`casts/*.yaml`); a consumer manifest references casts by name under `casts:` plus local include/exclude overrides. Resolution happens at install time against the pinned commit, so the same manifest and pin always produce the same selection.

### Consumer manifest

`.rune` at the target repo root. Entries reference a pinned source, optionally a `casts:` list, and rune ids under `runes:`. `rune add <id>[,<id>...]` and `rune add --cast <name>[,<name>...]` rewrite `.rune` atomically (temp file, rename), print the touched path, and print the install command; add does not install. Without `--source`, add uses the manifest's sole source, then `RUNE_DECK`, then the configured `deck` value. For local sources add resolves the whole selection eagerly, so unknown ids, unknown casts, and ambiguous short forms fail at add time listing candidates; git sources defer that check to install with a printed note. `rune install` resolves, assembles, and deploys the selection, including only hooks belonging to selected decks.

### Aggregate operations

Against a runedeck source, `validate`, `provenance`, `drift`, and `clean` iterate all decks and emit one aggregate report; a failure in any deck fails the run. `release` accepts a deck argument and packages that deck exactly as it packages a single-module source.

### Resolution semantics

- **Collision unit**: the assembled deploy-relative path per provider (after transforms). Runedeck validation fails on any duplicate across decks or across manifest sources; there is no precedence.
- **Short form**: `<deck>/<Name>` must be unique across all kinds within the deck; ambiguity is a hard error listing the candidates. A bare `<Name>` that is not a deck name must be unique across the whole runedeck, else the same error.
- **Per-entry resolution order**: casts (if referenced) → union with the entry's explicit ids → entry `exclude` last. Multiple manifest entries union; see collision rule.
- **`extends`**: parents resolve first, in listed order; pure set union; cycles are a hard error.
- **Globs** match canonical ids (`science/skills/Lit*`), `**` crosses segments, anchored to ids not paths. `exclude` removes ids regardless of whether a parent cast added them.
- **Deck identity**: the directory basename under `runes/`; `module.yaml` `name` must equal it or loading fails. `runes/` entries without `module.yaml` are skipped with a warning; nested `module.yaml` deeper than one level is ignored.
- **Pins**: commit SHAs, exactly as the git source machinery defines them; fetch or checkout failure is a hard error, never a fallback.
- **Install failure**: fail fast at the first failing deck; files already deployed in the run stay recorded in the target manifest (same contract as single-module install).
- **Hooks**: a deck's hooks deploy only when at least one of its runes is selected; hooks referencing another deck's files fail validation.
- **`rune add`** stores the requested form verbatim; adds are idempotent; a bare deck token selects every rune in that deck at resolve time.
- **Runedeck-level config**: only the provider list for v1; nearest scope wins per key.
- **Kinds are closed for v1** (`skills`, `agents`, `rules`, `hooks`); extending them is a spec revision.
- **Output ordering**: deck lexicographic, then kind (skills, agents, rules, hooks), then name lexicographic everywhere, so drift reports stay quiet.
- **Aggregate scope**: a source with `path:` behaves as a single module for every command; a runedeck-root source means all decks, for `validate`, `provenance`, `drift`, and `clean` alike. `release` against a runedeck requires the deck argument.

## Consequences

- Single-module sources keep their behavior, so the module-era tests stay green
- Deck deploys ship hook files and skill bundles verbatim and report every skipped file loudly
- Qualified ids make the consumer manifest self-documenting at the cost of longer entries
- Adding a deck is `mkdir` plus `module.yaml` with no registry edit; a malformed `module.yaml` in a new deck fails runedeck-wide validation immediately, which is intended
