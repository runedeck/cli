---
title: "Provider Detection Registry"
description: "One bundled registry with bounded evidence feeds setup, context, status, doctor, and drift"
type: adr
category: cli
tags:
    - cli
    - providers
    - detection
status: proposed
created: 2026-08-28
updated: 2026-08-28
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0013 Unified Config and Ontology"
    - "CLI-0019 Singular Noun Subcommands"
    - "CLI-0028 Setup Plan and Apply"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5", "gpt-5.6-sol"]
informed: []
upstream: []
---

# Provider Detection Registry

## Context and Problem Statement

Doctor and drift use fixed provider lists. Context treats directory existence as deployment.
Nothing tells a user which harnesses exist on the machine or why rune classified a provider.
[Herdr][HERDR] detects agents through data-driven manifests, updates them remotely, and explains
each classification on request. Rune needs deployment evidence, not terminal-screen evidence, and
its detection data influences where setup writes.

## Decision Drivers

- Setup, context, status, doctor, and drift must agree on the provider set
- Detection must never execute a harness
- Detection data influences write targets, so it must be trusted
- A user must be able to ask why a provider has its state

## Considered Options

1. **Keep fixed lists** — no new surface, but five commands keep diverging.
2. **Remote hot-updated manifests** — herdr's model. Flexible, but unauthenticated remote data
   would steer local writes.
3. **Bundled registry with bounded evidence** — one registry inside the signed release, read-only
   probes, one explain command.

## Decision Outcome

Option 3. One registry beside the embedded provider defaults feeds setup, context, status, doctor,
and drift. Evidence is bounded: an executable name on `PATH`, a known non-sensitive config
directory, a rune deployment manifest, and managed-file digest validation. Detection never executes
a harness and never accepts predicates from source-local config. `rune provider explain <NAME>`
prints the evidence list, the deployment state, and the fix command. The command lives under
`provider` because `agent` names artifact staging.

## Consequences

- [+] The fixed lists in doctor and drift disappear
- [+] Detection stays auditable and inside the release signature
- [-] New harness support requires a rune release
- [-] The registry must keep the protected `modified` state separate from `needs repair`

[HERDR]: https://github.com/herdrdev/herdr
