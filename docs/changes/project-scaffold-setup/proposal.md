---
adr: "docs/decisions/CLI-0028 Setup Plan and Apply.md"
status: proposed
---

# Scaffold Setup

## Why

`rune init` writes a project and stops. The next step, `make install`, is a manual one that a first-time user
misses, and the delta that describes the optional prompt has lived in this change directory without a proposal
or a task list since the feature merged (`4b8ae918`, "offer scaffold setup after init"). The change ledger reports
it as broken on every run of `rune spec doctor`.

## What changes

- After an interactive project init that wrote the Makefile, and only when `make` is available, Rune asks whether to
  run `make install`, defaulting to no.
- A noninteractive or JSON init never prompts and reports `make install` as the next manual step.
- A pre-existing Makefile is never run automatically.
- A failed setup keeps the successful scaffold and reports the manual step.

## Capabilities

- `project-init-command`

## Impact

- `src/cli/init/`: the prompt and the manual-step report.
- Documentation: the scaffold walkthrough names the prompt.
