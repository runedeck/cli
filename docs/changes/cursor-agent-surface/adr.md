---
title: "Cursor runs through its official agent CLI"
description: "rune run and rune launch reach Cursor only through cursor-agent in print mode with JSON output, map the run modes onto ask mode and Cursor's sandbox, and require an API key for clean runs"
type: adr
category: architecture
tags:
    - cli
    - run
    - launch
status: proposed
created: 2026-09-29
updated: 2026-10-02
author: "@N4M3Z"
project: cli
related:
    - "CLI-0024 Interactive and Automated Tool Commands"
    - "CLI-0025 Shared Coding Tool Process Supervisor"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude", "gpt-6-astra@codex", "grok-4.7@claude"]
informed: []
upstream: []
change: cursor-agent-surface
---

# Cursor runs through its official agent CLI

## Context and Problem Statement

The owner wants Cursor models in `rune run` and `rune launch`. Cursor offers two ways in. The official `cursor-agent` CLI has a print mode with one JSON result object. The editor talks to Cursor's backend over a private protocol, and Cursor's terms forbid access through it ([Cursor terms §1.5][TOS]). Claude models on Cursor bill at API price from Cursor's own pool ([Cursor models][MODELS]), so a Cursor route is an extra account, not a cheaper one.

The CLI differs from the other tools in four ways that shape the adapter. It has no system prompt flag. Its read-only mode, `--mode ask`, is enforced by the CLI and not by the operating system, and the model keeps its Shell and Write tools under it. It accepts options that its help does not list, such as `--data-dir` and `--allowed-tools`. It reads its login from the macOS Keychain, which a process inside the Claude Code sandbox cannot reach, and it reads `~/.cursor/rules` even when `CURSOR_CONFIG_DIR` points elsewhere. Each fact was checked against `cursor-agent` on 2026-09-29.

## Considered Options

1. Drive `cursor-agent -p --output-format json` as a sixth adapter, with the prompt on standard input.
2. Talk to Cursor's backend directly with a client for its private protocol.
3. Keep Cursor out of rune and let each user write a wrapper script.

For a clean run there were three further options: swap `HOME` and accept the lost login, set only `CURSOR_CONFIG_DIR`, or require `CURSOR_API_KEY`.

## Decision Outcome

Option 1. `rune run cursor` MUST start `cursor-agent` with `-p --output-format json --workspace <repo> --trust`, send the system prompt and the prompt on standard input, and read `result`, `is_error`, and `usage.outputTokens`. A read-only run MUST add `--mode ask` and MUST refuse when Cursor's saved `approvalMode` is `unrestricted`, one of the three values that Cursor documents ([Cursor CLI configuration][CONFIG]). A workspace-write run MUST add `--force --sandbox enabled`. A profile MUST keep only `-e` and `--endpoint`. `rune launch cursor` MUST start `cursor-agent` unless `tools.cursor.binary` says otherwise. A clean run MUST require `CURSOR_API_KEY`, set `HOME` and `CURSOR_CONFIG_DIR` inside the clean root, and refuse with a configuration error when the key is missing. `fable@cursor` MUST work without config: a built-in profile selects the built-in route `fable-cursor`, `claude-fable-5-1-high`, and a configured profile or route of the same name replaces it.

Option 2 breaks the terms and would break with every Cursor release. Option 3 is the wrapper that the 2026-09-26 retirement of vendor-named shims removed. For clean runs, a `HOME` swap alone fails with "Authentication required", and `CURSOR_CONFIG_DIR` alone still loads the owner's rules, so neither gives a clean run.

## Consequences

- Cursor models are reachable from `rune run` under the same owned-argument filter, JSON output, and failure kinds as the other tools.
- A read-only Cursor run relies on `cursor-agent` enforcing ask mode. Three live read-only runs on 2026-10-01 that asked for a file wrote nothing, under `approvalMode` `auto-review` and under a copied `unrestricted` config, and the model named its Shell and Write tools as present but forbidden. No kernel sandbox backs the mode, because `--sandbox enabled` without `--force` asks for approval that print mode cannot give. A caller who needs a kernel boundary for a read-only run must pick another tool. The refusal of `unrestricted` is defense in depth against that enforcement failing: an owner with that saved mode must choose `allowlist`, `auto-review`, or a workspace-write run.
- A Cursor profile cannot pass headers, `--api-key`, or any flag other than the endpoint flag. The key goes in the profile `env` as `CURSOR_API_KEY`. A profile that needs more must use `rune launch`, which passes profile arguments unchanged.
- `--clean` costs the owner an API key. The key lives in the environment, never in the argument list, so it does not appear in `ps` output or in the `rune run --dry-run` argument list.
- A clean run drops the owner's `~/.cursor/hooks`, the same as a clean run of any other tool drops its hooks.
- `cursor-agent` fails inside the Claude Code sandbox. rune adds a hint to that failure but does not leave the sandbox itself. The harness must exclude `rune`, and the owner's template must list `cursor-agent` for direct calls.
- The built-in profile table is new. It is the first profile that rune supplies without config, because Cursor names each Fable effort level as its own id and no tool-neutral `fable` route can serve it. Cursor models on Cursor's list carry the label "NO ZDR", so a `fable@cursor` prompt is not covered by zero data retention.
- Cursor does not document the `usage` block. If it changes, completion tokens become absent, and the run still succeeds.

[TOS]: https://cursor.com/terms-of-service
[MODELS]: https://cursor.com/docs/models
[CONFIG]: https://cursor.com/docs/cli/reference/configuration
