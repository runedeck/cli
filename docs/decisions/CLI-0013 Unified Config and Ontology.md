---
title: "Unified Config and Ontology"
description: "Rune uses one typed config file for ontology paths and extension roots"
type: adr
category: cli
tags:
    - cli
    - config
    - ontology
status: accepted
created: 2026-07-08
updated: 2026-07-08
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0011 Watchlist Monitored-Source Registry"
    - "RUST-0001 Structured Errors with ErrorKind"
    - "RUST-0006 Synchronous Core"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Unified Config and Ontology

## Context and Problem Statement

Rune commands need a shared vocabulary for user directories such as the quests, archive, vault, lore, mount, developer, artifacts, and git hooks locations. Rust commands cannot rely on shell functions for child process execution, extension lookup, or machine-readable config output. The kernel also needs to stay small: config loading should provide facts to scripts without absorbing script-level capabilities into Rust.

## Decision Drivers

- One typed source of truth for ontology values
- Deterministic precedence for environment overrides, config files, and defaults
- Pure library logic with no command output side effects
- No async, network, or dynamic config dependencies

## Considered Options

1. **Environment variables only.** This avoids a file, but gives extensions, launch, and watch sections no home and makes the effective state hard to display.
2. **Adopt `~/.config/rune/config.yaml`.** A typed user config can group ontology and extension roots while preserving deterministic environment override behavior.
3. **Use per-command config files.** Each feature could own a file, but scripts and external dispatch need the same ontology and would duplicate precedence rules.

## Decision Outcome

Chosen option: **Option 2**, because a single typed config gives the minimal Rust kernel enough shared facts to launch scripts and expose the resolved ontology while keeping behavior outside Rust.

Rune reads `~/.config/rune/config.yaml` into typed structs. The top-level config denies unknown fields so mistakes fail loudly, while reserved `launch` and `watch` sections remain parse-tolerant for future use. The top-level `deck` key supplies the default source for `rune add` and is set with `rune config set deck <path-or-url>`. Runtime accessors resolve `RUNE_*` environment variables first, config file values second, and built-in defaults last. Path-like ontology values expand a leading `~/` after resolution.

If `config.yaml` is absent, every key resolves from environment variables and built-in defaults; no other file is consulted.

## Consequences

- `rune config` can display the exact effective ontology, including provenance for each key.
- `RUNE_DECK` overrides the configured deck without editing the user config.
- `rune exec` and external dispatch inject the same `RUNE_*` values into child processes.
- Unknown top-level keys in `config.yaml` are errors, which prevents silent misspellings.
- Capability remains in scripts; Rust only loads config and reports resolved facts.

## More Information

- [CLI-0011 Watchlist Monitored-Source Registry](CLI-0011%20Watchlist%20Monitored-Source%20Registry.md)
- [RUST-0006 Synchronous Core](RUST-0006%20Synchronous%20Core.md)
