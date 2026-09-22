---
title: "Automated run drops the profile arguments it owns and keeps the rest"
description: "rune run filters a launch profile's arguments against the flags it sets itself, warns on each drop, filters codex config and claude settings per key, and never refuses the run for an overlap"
type: adr
category: architecture
tags:
    - cli
    - run
    - launch
status: proposed
created: 2026-09-22
updated: 2026-09-22
author: "@N4M3Z"
project: cli
related:
    - "CLI-0024 Interactive and Automated Tool Commands"
    - "CLI-0025 Shared Coding Tool Process Supervisor"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1", "gpt-6-astra", "grok-4.6"]
informed: []
upstream: []
change: run-accepts-profile-settings
---

# Automated run drops the profile arguments it owns and keeps the rest

## Context and Problem Statement

CLI-0024 gave `rune run` its own argument contract. It composes the noninteractive flags (print mode, output format, permission mode, tool allowlist, model, working directory) and rejects a profile that carries any of them, so a profile cannot widen the access the caller chose. The refusal was applied to whole flags. Codex `--config` and claude `--settings` are generic channels: one flag carries many keys, most of which the run does not set. The interactive launch needs those keys (`model_reasoning_effort`, the agent-teams switch under `env`). The result was a second profile per model with the arguments removed, and a run that still refused when someone used the wrong one. On 2026-09-22 `rune run astra@codex` refused and wrote nothing.

## Considered Options

1. Keep the refusal and keep one arguments-free profile per model for automation.
2. Drop each owned argument with a warning and pass the rest, with per-key filtering for `--config` and `--settings`.
3. Add a `--force-profile-args` flag that lets the profile win.

## Decision Outcome

Option 2. `rune run` MUST NOT refuse a profile for an argument overlap. For each argument the run owns it MUST drop the argument and its value and print one warning naming the surface, the flag, and the dropped value. For codex `-c`/`--config` it MUST filter per key and keep every key it does not set. For claude `--settings` it MUST filter per key, keep `env` and unknown keys, and remove the keys that change permissions, hooks, tools, plugins, or the model. An argument that widens access MUST never reach the tool.

Option 1 pushes the tool's limitation into every user's config and doubles the profile count. Option 3 is a footgun with a name: the point of `--mode read-only` is that nothing under it can widen access, and `--mode workspace-write` already exists for the other case.

## Consequences

- One profile per model serves `rune launch` and `rune run`. The `-run` twins go.
- A profile that sets a permission or a model for the run gets a warning instead of a refusal. The warning is on stderr and in the JSON `warnings` array, so an automation that cares can still fail on it.
- The owned tables become data in one place, which is where the next surface adds its list.
- A key the run does not list passes through, `env` included. The owned lists and the bypass table test are the boundary, and a new flag on any surface needs a table entry and a test row. Two adversarial reviews (astra, grok) pressed this point and were answered with the nested-prefix rule for codex, `sandbox` in the claude keys, and a refusal of `--settings` paths, not with an allowlist.
- Arity is per surface. `-p` is codex's profile and claude's print switch, so the boolean list is a table keyed by surface, and a following token that starts with `-` is never read as a value.
