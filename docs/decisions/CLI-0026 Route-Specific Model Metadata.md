---
title: "Route-Specific Model Metadata"
description: "Launch profiles select complete model routes that derive provider model and context settings together"
type: adr
category: cli
tags:
    - cli
    - launch
    - models
    - config
    - security
status: accepted
created: 2026-07-24
updated: 2026-07-24
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0013 Unified Config and Ontology"
    - "CLI-0018 Launch Middleware Chain"
    - "CLI-0021 Launch Profile Composition"
    - "CLI-0024 Interactive and Automated Tool Commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Route-Specific Model Metadata

## Context and Problem Statement

The same provider model can expose different context capacities through different endpoints or subscription routes. A profile that sets only a model identifier can leave the coding tool with stale context and compaction settings. Model identity and capacity therefore need one typed source that both interactive and automated execution resolve consistently.

## Decision Drivers

- Keep model identity and context capacity from the same route
- Allow one provider model to have different metadata through different endpoints
- Prevent profile environment entries from partially overriding generated model settings
- Keep endpoints, authentication, and optional small-model choices in profile environment
- Preserve redaction and provenance in dry-run output

## Considered Options

1. **Keep model and context variables as unrelated profile environment entries.** This is flexible but permits combinations that cannot describe a real route.
2. **Key metadata only by provider model identifier.** This cannot represent different context capacities for the same model through different routes.
3. **Define named model routes and let profiles select one complete route.** The route carries model identity, context capacity, and optional earlier compaction policy.

## Decision Outcome

Chosen option: **Option 3.**

`launch.models.<alias>` defines a model `id`, a `context` capacity, and an optional `compact` percentage. `launch.profiles.<tool>.<profile>.model` selects the alias. Configured routes replace complete built-in entries rather than merging individual fields.

For Claude Code, route selection derives `ANTHROPIC_MODEL`, `CLAUDE_CODE_MAX_CONTEXT_TOKENS`, and `CLAUDE_CODE_AUTO_COMPACT_WINDOW` as one group. `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` is emitted only when the route explicitly requests earlier compaction. A profile selecting a route must not also define these generated keys; Rune rejects the conflict instead of combining settings from different sources.

Endpoint and authentication values remain profile environment entries because they describe access to the route rather than model capacity. Optional small-model settings also remain in the profile. Both `rune launch` and `rune run` use the same resolved route and dry-run output reports the alias, model identifier, context capacity, source, generated settings, and redacted credentials.

## Consequences

- A route can assign different context capacities to the same provider model identifier.
- Claude Code receives model and compaction settings from one selected route.
- Existing profiles using raw generated environment keys must migrate before adding `model`.
- User-defined routes can replace built-in metadata without partial inheritance.
- Dry-run output makes the selected route and its source reviewable before execution.

## More Information

- [CLI-0013 Unified Config and Ontology](CLI-0013%20Unified%20Config%20and%20Ontology.md)
- [CLI-0018 Launch Middleware Chain](CLI-0018%20Launch%20Middleware%20Chain.md)
- [CLI-0021 Launch Profile Composition](CLI-0021%20Launch%20Profile%20Composition.md)
- [CLI-0024 Interactive and Automated Tool Commands](CLI-0024%20Interactive%20and%20Automated%20Tool%20Commands.md)
