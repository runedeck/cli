---
title: "Repair as the Single Write Path"
description: "Every doctor is read-only and names rune repair, the one command that restores, quarantines, prunes, and renames"
type: adr
category: cli
tags:
    - cli
    - doctor
    - repair
    - deploy
status: proposed
created: 2026-09-13
updated: 2026-09-13
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0027 Temporary Adoption Session State"
    - "CLI-0037 Subject Identity and Transfer Records"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
---

# Repair as the Single Write Path

## Context and Problem Statement

`rune doctor --repair` is part of release 0.5.0. It restored missing managed files from a
digest-matching build and quarantined deployment orphans under `.trash/`. `rune adopt doctor
--repair` printed a deprecation note and changed nothing (cli#21). Source-side faults, an
orphan reviewed sidecar or a stale subject name, had no writer at all. Two doctors carried
the same flag with two meanings, and one of them was a no-op.

## Decision Drivers

- A doctor is safe to run anywhere, because it never writes
- One command writes, with one dry run and one lock
- A reviewed subject whose bytes changed is a reseal, never a repair
- The 0.5.0 flag goes away with a changelog entry, not a silent alias

## Considered Options

1. **Per-doctor `--repair` flags**: the status quo, one more flag on `adopt doctor` with a
   real body.
2. **Top-level `rune repair`, every doctor read-only**: doctors report and print the repair
   command, `repair` runs the source pass and the deployment pass.
3. **Per-subsystem `--fix` sharing one module**: the same code behind two flags.

## Decision Outcome

Option 2.

`rune repair [--root <dir>] [--target <dir>] [--dry-run]` runs two passes. The source pass
moves orphan reviewed sidecars to `.trash/<stamp>/` and rewrites stale subject names under
the module at `--root`. The deployment pass, when a `.manifest` is discoverable under
`--target`, restores missing managed files from a digest-matching build and quarantines
managed-directory orphans, under the same per-target lock deploy holds. `--dry-run` prints
every write and changes nothing. The exit follows the findings that remain.

`rune doctor` and `rune adopt doctor` lose `--repair`. Neither aliases it. Each doctor names
`rune repair` when a finding is repairable and `rune adopt reseal --artifact <path>` when a
reviewed subject's digest changed, because endorsing edited bytes is a review act.

Reseal keeps the source-pass repair for the one artifact it endorses (CLI-0037): a reviewer
who reseals a moved artifact does not need a second command for its orphan or stale-name
sidecars. Repair is the only module-wide writer and the only command that acts on
deployment findings.

## Consequences

- A script that ran `rune doctor --repair` fails with a usage error until it moves to
  `rune repair`. The changelog lists the removal.
- Skill readiness inspection stays a doctor read. Its `--skill-readiness` flag no longer
  needs a conflict with `--repair`.
- Provider status hints and the agent guide name `rune repair`.
- A user-modified managed file is never touched by either command. `rune install --force`
  stays the only overwrite.
