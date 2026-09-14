# Repeatable artifact implementation

## Why

An implementation run can drift when workers change acceptance criteria or report results from an earlier candidate.
A repeatable procedure must bind its required checks to the reviewed contract and current input bytes.
Model reviews remain necessary for requirements that have no deterministic checker.

## What Changes

- Add a small local runner for reviewed validator commands and immutable result receipts.
- Require external pins for the runner and check manifest.
- Reject missing checks, skipped or empty test runs, timeouts, changed inputs, and changed executables.
- Add a reusable procedure for parallel review, bounded implementation, repair, and restart.
- Keep native observations and publication authority separate from local check results.

## Capabilities

### New Capabilities

- `artifact-implementation-loop`: Reproduce reviewed artifact checks and their repair procedure against identified inputs.

### Modified Capabilities

None.

## Impact

The CLI PR contains the runner, tests, example, and reusable procedure.
The separate Deck PR routes skill authoring through that procedure.
The existing source-layer validator supplies generic, harness, and model checks.
Existing repository hooks and signing rules remain authoritative.
The runner does not start model sessions, publish changes, or alter approval metadata.

The [implementation runbook](../../artifact-implementation-loop.md) defines the reusable procedure.
The [example contract](../../examples/artifact-checks.json) exercises the runner in a disposable checkout.
