---
title: "The sandbox guard refuses an exempt tool the sandbox would keep, and advises on chains"
description: "A built-in hook on the Claude Code shell tool refuses a sandbox-off request for a command that already runs outside the sandbox, refuses an exempt tool that the line keeps inside it with the command to run instead, and advises on joined commands"
type: adr
category: architecture
tags:
    - cli
    - hooks
    - sandbox
status: proposed
created: 2026-10-22
updated: 2026-10-22
author: "@N4M3Z"
project: cli
related:
    - "DOT-0007 Vendor Binaries on PATH and rune run for Automation"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-opus-5-5"]
informed: []
upstream: []
change: exempt-tool-is-the-whole-command
---

# The sandbox guard refuses an exempt tool the sandbox would keep, and advises on chains

## Context and Problem Statement

The owner's Claude Code sandbox exempts a list of tools from the sandbox, but only when every command on the line is one of them, with nothing before it and no redirection to a file. A probe on 2026-10-22 measured the rule. Models often write `cd /x && jj log`, `FOO=1 rune run …`, or `gh … | head`. Those lines run inside the sandbox, where codex and agy fail. Models also ask to turn the sandbox off for a command that already runs outside it, and each request is a prompt the owner must answer. The owner also finds joined commands hard to read and to debug.

## Considered Options

1. Refuse both mistakes with the command to run instead, and advise on joined commands that hold no exempt tool.
2. Refuse the sandbox-off request only, and advise on the rest.
3. Advise only, and refuse nothing.
4. Refuse every line that joins separate commands, for every tool.

## Decision Outcome

Option 1, chosen by the owner on 2026-10-22. The sandbox guard MUST refuse a sandbox-off request for a line that already runs outside the sandbox. It MUST refuse an exempt tool that the line keeps inside the sandbox, name what keeps it inside, and give the command to run instead. It MUST let any line without an exempt tool run, and it MUST add advice when that line joins separate commands.

Option 2 lets the wrapped lines run inside the sandbox and fail. Option 3 keeps the prompts. Option 4 can refuse a directory change that a tool with no directory option needs.

## Consequences

- A refused line costs the model one more turn. The command it then runs starts outside the sandbox with no prompt.
- The guard reads the harness's exempt list, so a change to the owner's settings needs no change to the guard.
- The guard knows Claude Code's rule as measured. A change in Claude Code can make it refuse a line that would run outside, or pass one that would not, until the guard is changed.
- The shell reader is not a full shell grammar. A line it misreads is refused or passed by mistake, and the refusal shows what it read.
