---
title: "The validator reports a program name in a requirement body"
description: "A requirement body names a role, and a program name has no place in it. The validator reports an inline code span that names a command or a flag, as a warning while the repository migrates and an error afterwards."
type: adr
category: process
tags:
    - cli
    - specs
    - lint
status: proposed
created: 2026-10-18
updated: 2026-10-18
author: "@N4M3Z"
project: cli
related:
    - "CORE-0023 Rune Fills the Tool Roles"
    - "CLI-0040 Specification House Rules"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude"]
informed: []
upstream: []
change: specifications-in-plain-language
---

# The validator reports a program name in a requirement body

## Context and Problem Statement

The owner decided in the voice review of 17 October 2026 that a specification names roles. A requirement says what happens and who does it, and a program name has no place in it. The actors are a validator, an assembler, a generator, a deployer, a fixer, a signer, and an archiver. The deck's record CORE-0023 Rune Fills the Tool Roles binds those roles to rune. Most requirements in the review, signing, and adoption changes of this repository name a command and its flags today, and nothing reports it. CLI-0040 put the house rules in a lint beside the compatibility parser, and this record adds one rule to that lint.

## Decision Drivers

- A requirement outlives a command name, so a rename changes no specification
- The rule holds in every repository that validates its specifications, whether or not the style checker runs there
- The names a body loses stay findable, in the change's design
- Existing changes keep validating while the repository migrates

## Considered Options

1. A style-checker rule alone. It sees prose, not requirement bodies, never fails specification validation, and runs only where the style checker runs.
2. A finding in the shared lint. The validator reports an inline code span in a requirement body whose first word is in the program list, or that starts with a dash. Severity comes from the repository configuration.
3. Review alone. A reviewer catches the names, and the next author writes them back.

## Decision Outcome

Option 2, with the style checker as the second net.

The validator MUST keep these rules:

- A code span in a requirement body MUST be reported when its first word is a listed program or it starts with a dash. Canonical specifications and deltas count alike.
- The finding MUST be a warning by default and an error once the repository configuration raises it.
- A span in a scenario step, a span in a fenced block, or a path with no command MUST NOT raise this finding. The style checker's bare-command rule covers those.
- Each name a body loses MUST appear in the change's design, in a commands table with one row per requirement.

The house rule covers more than the finding. Hook ids, paths, and diagnostic ids have no place in a body either, and review catches those until a check exists.

## Consequences

- Every active change whose requirements name a command moves the names into a design commands table during the migration.
- A literal that starts with a listed word is a false positive. The list is short, and the finding warns until the tree is clean.
- The program list needs upkeep. An unknown program passes until an author lists it in the repository configuration.
- Archive moves this record to `docs/decisions/` with the next free CLI number.
