---
title: "The run names the harness session where the harness takes a name"
description: "rune run --session-name passes a name to claude as --name and to opencode as run --title, passes nothing to codex, agy and grok, and reports in the plan and the JSON result whether the name went"
type: adr
category: architecture
tags:
    - cli
    - run
    - launch
status: proposed
created: 2026-10-19
updated: 2026-10-19
author: "@N4M3Z"
project: cli
related:
    - "CLI-0024 Interactive and Automated Tool Commands"
    - "CLI-0025 Shared Coding Tool Process Supervisor"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: run-names-the-session
---

# The run names the harness session where the harness takes a name

## Context and Problem Statement

A council mod starts one `rune run` per seat from a Claude Code session the owner named, `runedeck/mods/council`. The owner wants each seat's harness session named `<harness>/<that name>`, so a seat is found beside the session that asked. `rune run` composes each harness's command line itself and has no flag for a session name. Claude Code takes `--name <name>`. OpenCode takes `run --title <title>`. Codex `exec`, agy, and grok have no such flag. Each tool's help was read on 2026-10-19.

## Considered Options

1. One `--session-name <NAME>` on `rune run`, mapped per harness, reported as not sent where no flag exists.
2. An environment variable the harness reads. No harness reads one for this.
3. The caller passes the harness flag through the profile's arguments. The caller then has to know which harness a profile resolves to, and the run owns a profile's flags of that kind.

## Decision Outcome

Option 1. `rune run --session-name <NAME>` MUST pass the name as `--name` to claude and as `run --title` to opencode, and MUST pass nothing to codex, agy, and grok. The dry-run plan and the JSON result MUST name the flag that carried it, `none` when no flag did. A profile's own `--name`, `-n`, or `--title` MUST be dropped with a warning like every flag the run sets.

Option 2 has nothing to read it. Option 3 moves the harness table into every caller.

## Consequences

- A caller that needs the name on codex, agy, or grok reads `session_name_flag: none` and decides for itself.
- Under `--clean`, Claude Code's named session lives under a scratch config directory that the run removes, so the name is visible only while the seat runs.
- One more owned flag per harness that takes a name, each with a test row.
