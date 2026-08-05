---
title: "Shared Coding Tool Process Supervisor"
description: "Bench and automated coding-tool runs use one provider layer and one supervised process lifecycle"
type: adr
category: architecture
tags:
    - cli
    - bench
    - run
    - process
    - providers
status: accepted
created: 2026-07-24
updated: 2026-07-24
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0022 Native bench runner"
    - "CLI-0024 Interactive and Automated Tool Commands"
    - "RUST-0006 Synchronous Core"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Shared Coding Tool Process Supervisor

## Context and Problem Statement

The native benchmark runner and automated coding-tool command invoke the same provider programs with the same prompt, sandbox, output, and cleanup requirements. Separate adapters or process loops would drift in provider flags, semantic error parsing, timeout behavior, and child cleanup. The shared implementation must remain synchronous and must not require unsafe code.

## Decision Drivers

- Keep one provider contract for bench and `rune run`
- Distinguish normal exit, signal exit, timeout, output limits, and supervisor failures
- Drain stdout and stderr without allowing a child to block on full pipes
- Forward terminal interruption to the supervised child process group
- Preserve no-timeout execution when callers do not request a deadline
- Return errors from process input and output instead of replacing them with empty data

## Considered Options

1. **Keep a process loop inside each command.** This reduces initial refactoring but duplicates lifecycle behavior and provider adapters.
2. **Use shell timeout and redirection wrappers.** This cannot provide consistent process-group cleanup, typed failures, or provider event parsing.
3. **Share one Rust provider layer and process supervisor.** Bench and `rune run` construct requests for the same implementations.

## Decision Outcome

Chosen option: **Option 3.**

The shared provider module owns provider request construction, prompt transport, structured output parsing, and semantic failure handling for Claude, Codex, agy, Grok, and OpenCode. Bench runners and `rune run` both call this module.

Each child starts in its own Unix process group. Standard output and standard error are drained concurrently. Capture is bounded, but readers continue consuming excess bytes so the child cannot block. Crossing the limit terminates the supervised process group and returns an output-limit failure with the retained tail.

Timeouts are optional. On a requested timeout or forwarded SIGINT or SIGTERM, Rune sends the initiating signal or SIGTERM to the child process group, waits through a grace interval, sends SIGKILL when necessary, and reaps the direct child. Cleanup is best effort for descendants that remain in the direct child's process group. Descendants that create another process group or session are outside this guarantee.

Bench supplies its benchmark timeout. `rune run` supplies no timeout by default. agy receives a requested deadline through its native print option and a later supervisor deadline. Other providers receive only the optional supervisor deadline requested by the caller.

## Consequences

- Bench and `rune run` cannot silently diverge in provider invocation behavior.
- OpenCode session errors fail the request even when the process exits successfully.
- Output-heavy children do not deadlock the parent after capture reaches its limit.
- Terminal interruption reaches the full supervised process group before forced cleanup.
- tmux sessions and daemon-controlled Docker processes remain unsupported in automated execution.

## More Information

- [CLI-0022 Native bench runner](CLI-0022%20Native%20bench%20runner.md)
- [CLI-0024 Interactive and Automated Tool Commands](CLI-0024%20Interactive%20and%20Automated%20Tool%20Commands.md)
- [RUST-0006 Synchronous Core](RUST-0006%20Synchronous%20Core.md)
