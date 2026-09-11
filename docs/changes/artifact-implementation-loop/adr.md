---
title: "Bind artifact checks to reviewed inputs"
description: "Record bounded artifact checks against externally identified contracts and current input bytes."
type: adr
category: cli
tags: [artifacts, validation, agents]
status: proposed
created: 2026-09-11
updated: 2026-09-11
author: "@N4M3Z"
project: rune-cli
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["gpt-6-astra"]
informed: []
upstream: []
---

# Bind artifact checks to reviewed inputs

## Context and Problem Statement

A model can produce a plausible completion report while omitting checks or using stale evidence.
Instructions alone cannot establish that required commands ran against the current artifact.
The existing publication wrapper validates an immutable commit but does not define each artifact's acceptance work
order.

This ADR remains an unnumbered proposal pending maintainer review.

## Considered Options

1. A prose checklist alone cannot detect missing or stale execution records.
2. A new model orchestration service would add deployment and harness dependencies to a local validation problem.
3. A zero-exit-only command runner would accept empty test runs and required skips.

## Decision Outcome

Use one reviewed JSON manifest to name required validators, inputs, execution limits, and result adapters.
Run commands as argument arrays without a shell.
Require the coordinator to supply expected manifest and runner hashes from outside the worker's scope.
Record input and executable hashes before and after checks.
Store receipts outside candidate source.
Use independent agents to review the contract, implementation, and uncovered semantic requirements.
Reuse existing validators and the repository's immutable publication checks.

## Consequences

- A receipt describes observed checks for a particular input inventory.
- The manifest must include all inputs and commands relevant to its claims.
- The runner cannot prove semantic completeness or protect itself from a worker with verifier write access.
- Native runtime evidence and maintainer approval remain separate.
