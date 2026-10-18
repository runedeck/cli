# Artifact implementation loop design

## Context

The current work already has scoped agents, OpenSpec requirements, an ADR, static validators, and immutable
publication checks.
The missing reusable part is a check contract and a receipt that a fresh session can reproduce.

## Goals / Non-Goals

Make each implementation run restartable from an explicit contract and recorded failure.
Detect missing execution, invalid test evidence, source changes, and stale checker identities.
Do not build a model scheduler or claim that static success proves native behavior.
Do not replace existing hooks, signing, deployment approval, or repository ownership.

## Decisions

A coordinator freezes the requirement map, worker paths, reviewer paths, and check manifest before implementation.
The manifest selects explicit command arguments and result adapters.
The runner verifies externally supplied pins and records the declared input inventory.
Commands use explicit deadlines and output limits.
The runner rejects missing executables, malformed output, zero test counts, required skips, and changed inputs.
A command-only lint result remains distinct from counted test evidence.
Receipts remain outside source and identify the runner, manifest, executables, inputs, and observed results.

Independent reviewers assess spec, ADR, rule, agent, and skill consistency.
Workers receive concrete findings and return to implementation.
A contract change returns to contract review and invalidates earlier acceptance results.
An implementation change repeats affected checks and the final mandatory checks.
A timeout, unavailable tool, or failed native observation remains unresolved.

Computer use can collect native evidence when a supported machine interface is unavailable.
The reviewer must identify the harness, candidate, actual action, and observed result.
A screenshot or model answer alone cannot prove complete discovery or companion access.

## Risks / Trade-offs

Input coverage and checker semantics depend on review.
A receipt does not prove independence when the worker can change the verifier or its expected hashes.
Declared command adapters support finite output formats and must reject unsupported output.
The runner executes reviewed local commands and is not a sandbox.

## Migration Plan

1. Add and test the runner against disposable valid and invalid fixtures.
2. Publish the runbook and an example contract.
3. Route skill authoring through the runbook in the separate content PR.
4. Require a reviewed contract and external verifier ownership for future implementation runs.

## Commands

The delta already names roles (the coordinator, the runner, the reviewers, the validators) and no
command, flag, or path moved out of it. This table records what implements each requirement, as
the [runbook](../../evidence-cannot-go-stale.md) describes it.

| Requirement | Command and flags | Hook | Diagnostic ids and files | Exit status |
|---|---|---|---|---|
| The run starts from a reviewed contract | `python3 scripts/run-artifact-checks.py freeze --root <DIR> --manifest <FILE> --runner-sha256 <SHA> --manifest-sha256 <SHA> --output <FILE>`. The example manifest is `docs/examples/artifact-checks.json`. | none | "The reviewed pin does not match" | 1 on a pin mismatch |
| Required execution fails closed | `python3 scripts/run-artifact-checks.py run ... --snapshot <FILE> --snapshot-sha256 <SHA>`. Adapters are `command` (exit zero only, no test count) and the counted test adapters. | none | missing executable, deadline expired, zero tests counted | 1 when the receipt is not accepted |
| Receipts bind to current inputs | The receipt is the runner's output file outside the candidate source. | none | changed input, changed executable | 1 on a change during validation |
| Repair and restart preserve evidence limits | The procedure in the runbook. No command. | none | none | none |
