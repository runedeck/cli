---
title: "Interactive and Automated Tool Commands"
description: "Rune keeps terminal sessions, supervised coding-tool execution, and skill scripts on separate commands"
type: adr
category: cli
tags:
    - cli
    - launch
    - run
    - exec
status: accepted
created: 2026-07-24
updated: 2026-07-24
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0014 Exec Runtime Contract"
    - "CLI-0018 Launch Middleware Chain"
    - "CLI-0021 Launch Profile Composition"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Interactive and Automated Tool Commands

## Context and Problem Statement

Coding tools need two different process contracts. A person opening an interactive terminal session expects inherited input and output, native argument forwarding, and terminal features such as session resume. Automation needs captured output, explicit sandbox policy, provider-specific prompt transport, signal handling, and optional deadlines. Skill-bundled scripts have a separate resolution and runtime contract.

## Decision Drivers

- Preserve the native interactive behavior and argument surface of coding tools
- Give automation typed output and process lifecycle failures
- Reuse launch profiles, model metadata, middleware, and preflight checks
- Keep skill scripts within the accepted exec runtime contract
- Prevent terminal and daemon wrappers from escaping automated process supervision

## Considered Options

1. **Add a headless flag to `rune launch`.** One command would select incompatible input, output, and process behavior through flags.
2. **Route coding tools through `rune exec`.** This would mix installed provider programs with scripts bundled inside skills.
3. **Keep separate `launch`, `run`, and `exec` commands.** Launch resolution is shared, while each command retains the execution contract its callers need.

## Decision Outcome

Chosen option: **Option 3.**

`rune launch [profile@]<tool>` opens an interactive coding-tool session. It inherits terminal input and output, executes through `Command::status()`, forwards native arguments unchanged, and retains terminal wrappers such as tmux.

`rune run [profile@]<tool>` resolves the same tool, profile, model route, middleware environment, and preflight plan. It then executes through the supervised provider path with a prompt from an argument, `--prompt-file`, or noninteractive stdin. Read-only mode is the default; workspace-write mode is explicit. No timeout is applied unless the caller requests one.

Automated execution rejects tmux and Docker wrappers because their processes are controlled outside the direct child process group. The error directs the caller to an unwrapped profile or to `rune launch`.

`rune exec <skill>` remains limited to scripts bundled with skills and keeps the runtime contract defined by CLI-0014.

## Consequences

- Interactive sessions retain native terminal behavior, including provider resume options.
- Automated callers receive captured final output and typed failures without duplicating launch configuration.
- Provider-specific noninteractive flags stay out of profiles and orchestration skills.
- A profile intended for both commands must avoid wrappers that automated execution cannot supervise.

## More Information

- [CLI-0014 Exec Runtime Contract](CLI-0014%20Exec%20Runtime%20Contract.md)
- [CLI-0018 Launch Middleware Chain](CLI-0018%20Launch%20Middleware%20Chain.md)
- [CLI-0021 Launch Profile Composition](CLI-0021%20Launch%20Profile%20Composition.md)
- [CLI-0025 Shared Coding Tool Process Supervisor](CLI-0025%20Shared%20Coding%20Tool%20Process%20Supervisor.md)
- [CLI-0026 Route-Specific Model Metadata](CLI-0026%20Route-Specific%20Model%20Metadata.md)
