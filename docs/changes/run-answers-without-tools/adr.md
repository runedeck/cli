---
title: "An answer-only run gives the harness no tool at all"
description: "rune run --mode answer-only starts claude with --tools empty and opencode with every permission denied, and refuses codex, agy and grok, which cannot run without tools; the mode exists for a council seat the owner does not trust with the repository"
type: adr
category: architecture
tags:
    - cli
    - run
    - trust
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
change: run-answers-without-tools
---

# An answer-only run gives the harness no tool at all

## Context and Problem Statement

The council mod runs its seats in three tiers of trust. A red seat is a model the owner does not trust with the repository: it gets one public paragraph, runs clean, and must learn nothing else. The run's read-only mode leaves the harness its Read, Glob and Grep tools, and a harness with a Read tool reads any path it is told or guesses. On 2026-10-19 a grok seat opened an earlier run's record under `.rune/records` to recover a question. For a red seat that door has to be shut by the run, not by the brief.

## Considered Options

1. A third access mode, answer-only, that each harness maps to its own no-tools switch, with a refusal where none exists.
2. A separate `--no-tools` flag beside the mode. Every reader of the mode would also read the flag, and the two could disagree.
3. An empty working directory alone. The tools still reach absolute paths.

## Decision Outcome

Option 1. `rune run --mode answer-only` MUST start claude with `--tools ""` and opencode with every permission denied, MUST refuse codex, agy and grok before anything starts, naming the harnesses that can, and MUST print the mode in the dry-run plan.

## Consequences

- A red seat needs a claude or opencode profile. The council's default roster puts kimi on claude.
- A harness that later learns to run without tools gets a one-line change in the refusal.
- The mode removes the built-in tools. Plugin and MCP tools are a settings matter, and a clean run loads no settings.
