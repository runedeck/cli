---
title: "Built-in Claude Code profiles route through a local CLIProxyAPI"
description: "Rune defines sol@claude and grok@claude without any user config. Both send Claude Code to the configured CLIProxyAPI endpoint with the token from CLIPROXY_API_KEY, and a configured profile of the same name replaces each one."
type: adr
category: cli
tags:
    - cli
    - launch
    - profiles
status: proposed
created: 2026-08-14
updated: 2026-10-26
author: "@N4M3Z"
project: cli
related:
    - "CLI-0021 Launch Profile Composition"
    - "CLI-0026 Route-Specific Model Metadata"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream:
    - "convenient-cross-harness-launcher#named-launch-profiles"
    - "https://github.com/runedeck/cli/pull/24"
change: convenient-cross-harness-launcher
---

# Built-in Claude Code profiles route through a local CLIProxyAPI

## Context and Problem Statement

A launch profile lives in the user config, so a fresh installation has none. A user who wants Claude Code on Sol or Grok through CLIProxyAPI ([CLIPROXY]) writes the same profile by hand: the base URL, the token reference, the small model, the model route, and the `cliproxy` readiness check. The `launch` help text and `config.example.yaml` show that profile as a template. The question is whether rune defines these two profiles itself.

## Considered Options

1. Templates only. Rune documents a commented profile, and each user writes it into the config.
2. Built-in profiles over the configured endpoint. Rune defines `sol@claude` and `grok@claude` over the endpoint that `launch.middleware.cliproxy` names, and a configured profile of the same name replaces each one.
3. Built-in profiles over a fixed endpoint. Rune writes `http://127.0.0.1:8317` into each profile.

## Decision Outcome

Option 2.

- The built-in catalog MUST define `sol@claude` and `grok@claude` for Claude Code. Each MUST select the built-in model route of its name.
- Each built-in profile MUST set `ANTHROPIC_BASE_URL` from the `launch.middleware.cliproxy` host and port, `http://127.0.0.1:8317` by default. Each MUST add the `cliproxy` middleware, so the launch checks that the port listens.
- Each MUST set `ANTHROPIC_AUTH_TOKEN` from a `from_env` reference to `CLIPROXY_API_KEY`. The catalog MUST NOT hold a token value.
- A configured profile MUST replace the built-in profile of the same name as a whole. The other built-in profile MUST stay.
- Bare `rune launch` MUST list the built-in profiles beside the configured ones.

Option 3 ties each profile to one port, so a user whose proxy listens elsewhere copies the whole profile to change one value. Option 1 leaves a fresh installation without a cross-model launch and puts the same block into every user's config.

## Consequences

- `rune launch sol@claude` works on a fresh installation once CLIProxyAPI listens and `CLIPROXY_API_KEY` is set.
- One config key, `launch.middleware.cliproxy`, moves the base URL and the readiness check together.
- Rune names two proxy profiles and their model routes in code. A model rename at the proxy needs a rune release or a configured override.
- The profiles set `ANTHROPIC_SMALL_FAST_MODEL`, which the Claude Code reference marks deprecated ([CCENV]).
- Without `CLIPROXY_API_KEY` in the environment or the env file, a built-in profile fails before launch, `--dry-run` included.
- CLI-0021 states that rune gives commented templates and no endpoints. This record holds the opposite for the two Claude Code profiles, so CLI-0021 needs an amendment.

[CCENV]: https://code.claude.com/docs/en/env-vars
[CLIPROXY]: https://github.com/router-for-me/CLIProxyAPI
