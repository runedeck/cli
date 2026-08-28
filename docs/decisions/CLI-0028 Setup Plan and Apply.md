---
title: "Setup Plan and Apply"
description: "The setup wizard prints a write plan, applies after one approval, and records versioned completion after verification"
type: adr
category: cli
tags:
    - cli
    - ux
    - setup
status: proposed
created: 2026-08-28
updated: 2026-08-28
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0003 Conflict Resolution on Install"
    - "CLI-0007 Interactive Mode and TUI"
    - "CLI-0029 Structured Repair Errors"
    - "CLI-0030 Provider Detection Registry"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5", "gpt-5.6-sol"]
informed: []
upstream: []
---

# Setup Plan and Apply

## Context and Problem Statement

`rune setup` configures a deck and stops. No command routes a new user into it.
It selects no providers, verifies nothing, and keeps no completion state.
A study of [herdr v0.8.2][HERDR] showed a first-run flow that sends users from a welcome step to
integration installs. Herdr stores one Boolean and marks onboarding complete before the installs run.
A wizard for rune writes into the user config and into harness directories.
Those writes need review, verification, and a durable record.

## Decision Drivers

- No silent writes into user config or harness directories
- Automation needs the same flow without prompts
- Completion must mean verified, not attempted
- The flow must work without the `tui` feature

## Considered Options

1. **Boolean first-run flag with immediate apply** — herdr's model. Simple, but it marks completion
   before verification and hides the writes.
2. **Plan-then-apply wizard with a versioned setup record** — the wizard prints every planned write,
   asks once, applies, verifies, and only then records completion with a version number.
3. **No wizard, documentation only** — no new code, but new users keep failing at the first
   unconfigured command.

## Decision Outcome

Option 2. `rune setup` prints the full write plan and asks for one approval.
`rune setup --plan --json` performs no writes. `rune setup --yes` applies detected defaults after it
prints the plan. A versioned `setup` record in the typed user config replaces a Boolean, so a later
CLI version can re-run only the missing steps. Completion state is written only after every selected
verification check passes. Bare `rune` without a user config prints one `next: rune setup` line and
writes nothing.

## Consequences

- [+] Every write is visible before it happens and verified after it happens
- [+] Automation and agents consume the identical plan through `--plan --json`
- [+] A version number gives setup a resume and upgrade path
- [-] The wizard needs the structured errors of CLI-0029 for its verification output
- [-] More prompts than herdr's flow for a first-time user

[HERDR]: https://github.com/herdrdev/herdr
