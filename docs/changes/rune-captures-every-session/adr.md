---
title: "Rune captures harness sessions itself, from the hook dispatcher, the runner, and a schedule"
description: "rune session capture and rune session sweep replace the session-sync script, drop OpenCode and Gemini, keep versions only for lost bytes, and record capture debt only for a session file that failed to copy"
type: adr
category: architecture
tags:
    - cli
    - session
    - capture
    - hooks
status: proposed
created: 2026-11-02
updated: 2026-11-02
author: "@N4M3Z"
project: cli
related:
    - "CLI-0024 Interactive and Automated Tool Commands"
    - "CLI-0025 Shared Coding Tool Process Supervisor"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-opus-5-5"]
informed: []
upstream: []
change: rune-captures-every-session
---

# Rune captures harness sessions itself, from the hook dispatcher, the runner, and a schedule

## Context and Problem Statement

The owner keeps every harness session in `~/Data`, so that a session can be read, searched, and loaded into analysis later. A shell script, `session-sync`, does the copy. It runs from an unmerged workspace of forge-data, needs bash, jq, rsync, python3, and SpecStory, and has three callers: a Claude Code hook, a launcher mode that nothing calls, and an hourly launchd job. On 2026-11-02 it had 767 open debt records that nothing clears, and its version store held 50 GB of hourly OpenCode database copies. The owner decided the same day that capture becomes part of rune and that rune stops capturing OpenCode.

The runner knows facts the script cannot see: which harness ran, its session id when the harness reports one, and whether the run was clean. The owner decided on 2026-11-02 that a clean run is not captured by default, that work sessions are excluded by configuration, and that each host captures its own sessions.

## Considered Options

1. Keep the script and fix it in place: drop OpenCode and Gemini, stop the SpecStory step for Codex, reconcile debt.
2. Move capture into rune: a session module that the hook handler, the runner, and the launchd job call. SpecStory stays an optional renderer for Claude Code.
3. Hand capture to an external tool: SpecStory alone, or each harness's own export.

## Decision Outcome

Option 2. Rune MUST copy the session files of Claude Code, Codex, Grok, and Antigravity into the data folder, from three triggers, and the optional sources only when the owner enables them. Rune MUST skip a session that the exclude configuration names, by working folder glob or by session id, without debt. Rune MUST NOT capture a clean run by default. Rune MUST keep a version only for lost bytes and at most one database backup per day. Rune MUST record capture debt only for a session file that exists and failed to copy, and MUST clear it on a later success or a complete copy. Each host captures its own sessions.

Option 1 keeps a second runtime on every host and a PATH link into an unmerged workspace, and the script cannot tell a work session from a private one. Option 3 does not work for Codex: SpecStory finds sessions by working folder and failed for 350 Codex sessions, and it writes Markdown, not the native files.

## Consequences

- Rune grows a module that copies files. Its tests need fixture homes for each harness layout, and a harness that changes its layout breaks the copy until the fixture and the code follow.
- Codex sessions get no readable copy after the switch. The native files stay complete, and existing Markdown copies stay.
- OpenCode sessions after the switch exist only in OpenCode's own database.
- The first sweep closes most of the old debt records at once. The debt count drops in one step, which is the cleanup, not a sign of lost data.
- A failed capture no longer fails a launcher call, as the script's exit 1 did. It becomes one warning and a debt record.
- A clean run leaves no session copy. Its temporary home goes with the run, as the run change says.
- A work session that the exclude configuration misses is copied. The owner widens the globs, and the copy stays.
