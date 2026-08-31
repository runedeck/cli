---
title: "Structured Repair Errors"
description: "Recoverable errors carry a stable code and a fix command, rendered once at the CLI edge"
type: adr
category: cli
tags:
    - cli
    - ux
    - errors
status: proposed
created: 2026-08-28
updated: 2026-08-28
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0004 Structured Operation Results"
    - "CLI-0028 Setup Plan and Apply"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5", "gpt-5.6-sol"]
informed: []
upstream: []
---

# Structured Repair Errors

## Context and Problem Statement

Rune errors carry a kind and a message. The CLI prints one `fatal:` line.
Some messages embed repair advice, most do not, and JSON consumers cannot match errors reliably.
[Herdr][HERDR] builds each error with a stable code and the exact command that repairs the state,
for example "no herdr server is running at PATH; run `herdr` to start or attach it".

## Decision Drivers

- A user in a broken state needs the next command, not a description
- Agents and scripts need stable identifiers, not message matching
- Repair text must never contain placeholders

## Considered Options

1. **Message conventions only** — ask authors to include repair advice in message text.
   Unenforceable and invisible to JSON consumers.
2. **Structured code and fix command** — extend the error type with a stable `code` and an optional
   `fix_command`, and render once at the CLI edge.
3. **Per-command ad hoc JSON** — each command shapes its own error output. Divergent and unstable.

## Decision Outcome

Option 2. `src/error.rs` gains a stable `code` and an optional `fix_command`.
The CLI renders the error once. Human output prints the fix on its own line.
JSON output carries `code`, `message`, and `fix_command`.
Fix commands are built from resolved paths and provider names, never placeholders.
Each recoverable error requires a fix command or a precise next check.
The first adopters are setup, config, provider, install, and doctor errors.

## Consequences

- [+] Every recoverable failure names its repair
- [+] Stable codes give agents and tests a contract
- [-] Existing error sites need an audit and migration
- [-] A wrong fix command is worse than none, so the commands need tests

[HERDR]: https://github.com/herdrdev/herdr
