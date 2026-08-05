---
title: "Launch Profile Composition"
description: "Named launch profiles compose with the CLI-0018 middleware chain instead of replacing it"
type: adr
category: cli
tags:
    - cli
    - launch
    - profiles
    - security
status: accepted
created: 2026-07-19
updated: 2026-07-24
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0018 Launch Middleware Chain"
    - "CLI-0024 Interactive and Automated Tool Commands"
    - "CLI-0026 Route-Specific Model Metadata"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Launch Profile Composition

## Context and Problem Statement

`rune launch` composes environment middleware (CLI-0018): an ordered chain of plan patches with `--with`, `--direct`, `--tmux`, and `--dry-run`. Users also want named presets — "launch Claude Code pointed at a different model or endpoint" — selectable per invocation (`rune launch sol@claude`, profile@tool like user@host). A preset mechanism could either replace the middleware chain with a profile-only launcher or layer on top of it.

## Decision Drivers

- CLI-0018 is accepted and carries a script extension contract; discarding it breaks existing middleware
- Cross-vendor overrides must be pure environment configuration; rune manages no proxies
- Secrets must never live in config files or appear in launch output
- Untrusted repositories must not be able to redirect credentials through launch configuration

## Considered Options

1. **Profile-only launcher**: replace the middleware chain with named env presets.
2. **Profiles compose with the chain**: a profile contributes `env`, `args`, and `with` entries; the chain machinery stays authoritative for plan composition.

## Decision Outcome

Chosen option: **Option 2.**

- `rune launch [profile@]<tool>` resolves `launch.profiles.<tool>.<name>` from the user config. A profile carries `env:` (map), `args:` (prepended to tool args), and `with:` (middleware appended to the chain).
- Env values are literals or `from_env: KEY` references resolved at launch: the parent environment wins, an unset variable falls back to the env file (config key `env`, default `~/.env`), and a reference absent from both is a hard error naming the file. Secrets are stored as references, never values.
- Profiles resolve from the user config only. Repo-level profile definitions stay out until a restricted merge exists that forbids credential, endpoint, proxy, certificate, `PATH`, loader, `HOME`, and `XDG_*` keys from repository sources.
- Bare `rune launch` lists known tools with install state and defined profiles.
- For `ollama`, a profile name with no matching profile is a model: `rune launch llama3@ollama` dispatches `ollama run llama3`.
- The pre-exec freshness warning (deployment older than the deck) waits until deploy records the source commit in the manifest; age heuristics were rejected.

## Consequences

- Existing middleware configuration and flags keep working unchanged; profiles are additive.
- Dry-run output redacts values of credential-marker keys (`KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `CREDENTIAL`) in both env lines and wrapped argv; `SENSITIVE_ENV_KEYS` separately blocks middleware from setting loader variables.
- Cross-vendor launches (Sol inside Claude Code) become a profile with `ANTHROPIC_BASE_URL` + model env pointing at infrastructure the user runs; rune ships commented templates, not endpoints.
