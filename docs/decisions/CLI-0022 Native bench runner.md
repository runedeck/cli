---
title: "Native bench runner"
description: "rune bench reimplements the SkateBench-compatible benchmark runner natively in Rust instead of wrapping the bun harness, keeping rune a single static binary while staying byte-compatible with existing cache and results formats."
type: adr
category: architecture
tags:
    - bench
    - cli
    - parity
status: proposed
created: 2026-07-18
updated: 2026-07-24
author: "@N4M3Z"
project: rune
related:
    - "CLI-0021 Launch Profile Composition"
    - "CLI-0025 Shared Coding Tool Process Supervisor"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Native bench runner

## Context and Problem Statement

The benchmark harness lives in runedeck/bench as a bun/TypeScript runner that
scores models and agentic CLI harnesses on QA and judged suites. Bench is a
component of rune, but running it requires a bun toolchain and a repo checkout,
while every other rune capability ships inside one static binary. How should
`rune bench` deliver the runner?

## Decision Drivers

- rune installs as a single binary with no runtime dependencies.
- Existing results and cache directories must stay resumable; the formats are
  specified byte-exactly in runedeck/bench `docs/skatebench-compat.md`.
- The suite format tracks upstream SkateBench (T3-Content/skatebench, MIT), so
  compatibility is a contract, not an implementation detail.

## Considered Options

1. **Wrap the bun harness**: rune shells out to a discovered bench checkout.
   Cheapest, but bun and a second repo join the critical path of a rune command.
2. **Auto-fetch the harness** into a cache at a pinned version. Same runtime
   dependency plus network and pinning machinery.
3. **Port the runner to Rust** inside rune, against the verified protocol
   contract; the bun harness remains as reference until parity is proven, then
   retires.

## Decision Outcome

Option 3. The protocol is small, fully specified, and already verified against
upstream source; the data (suites, models.yaml, results) stays in the bench
workspace, and rune brings the runner to it. Parity is proven by running both
runners on identical fixtures and diffing outputs, plus cross-runner resume in
both directions.

## Consequences

- rune gains dependencies for blocking HTTP and SHA-1 and a `src/cli/bench/`
  module tree; the binary stays free of async runtimes.
- Two implementations coexist until the parity check passes; the bun harness is
  then reference-only and drops out of the workflow.
- Format changes now start in `docs/skatebench-compat.md`, not in code, since
  two consumers depend on the byte layout.
- The held-out tier renames from `suites/sealed/` to `suites/private/` as part
  of the same change.
