---
title: "Overseer Interfaces"
description: "rune exposes a terminal UI and a read-only web dashboard over one shared services layer."
type: adr
category: cli
tags:
    - tui
    - dashboard
    - overseer
status: accepted
created: 2026-07-13
updated: 2026-07-13
author: "Grok (grok-composer-2.5-fast)"
project: rune
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["HarnessCouncil"]
informed: []
upstream: []
---

# Overseer Interfaces

## Context and Problem Statement

rune manages deck state, casts, and deployment history across domains and kinds. CLI commands alone make cast composition and history inspection tedious at catalog scale. Operators need a primary terminal experience and a lightweight browser view for read-only inspection. Two faces must not diverge in behavior or mutate state through paths the CLI cannot reach.

## Considered Options

- CLI subcommands only, no interactive surface
- One terminal UI as the sole overseer
- Terminal UI plus a read-only loopback dashboard over one shared services layer
- A read-write web application as the primary interface

## Decision Outcome

rune ships two interfaces over one deck-state layer.

**Terminal UI (primary):** vim-keybinding interaction modeled on tuicr; Miller columns for domain, kind, and status navigation; cast composition workflows; a history view following gitui and jjui patterns with batched background log walking, a sliding metadata window, bindings mapped to actions rather than hardcoded keys, and jj graph glyphs parsed rather than hand-drawn.

**Read-only web dashboard:** axum, htmx, and askama on loopback; inspection only.

Both consume the same services layer as CLI commands. Neither mutates state the CLI cannot perform.

gitui and jjui are MIT-licensed. Vendoring specific components with attribution is permitted; adopting patterns is preferred over wholesale copies.

## Consequences

- Operators get fast keyboard-driven navigation plus a browser view for casual inspection without learning the full CLI
- Shared services keep behavior consistent across surfaces
- Two interfaces cost more to build and maintain than a CLI-only tool
- The TUI carries the complexity of log walking and graph rendering; write workflows stay in the terminal because the dashboard is read-only
- Vendored components need license attribution and periodic alignment when upstream UI projects change
