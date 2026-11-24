---
title: "Harness Interfaces"
description: "The CLIProxyAPI hub setup from Theo's guide is the baseline for every interface, and rune fills only the gaps it leaves: the client step from launch profiles, the cache lifetime, T3 Code model names, and checks"
type: adr
category: cli
tags:
    - cli
    - launch
    - profiles
    - interfaces
    - cliproxyapi
status: proposed
created: 2026-11-02
updated: 2026-11-02
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0021 Launch Profile Composition"
    - "CLI-0024 Interactive and Automated Tool Commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-opus-5-5"]
informed: []
upstream:
    - "https://gist.github.com/t3dotgg/59a521d2ee8d70aaf90e280c175072af"
---

# Harness Interfaces

## Context and Problem Statement

The owner starts and watches most harness sessions in T3 Code, and Multica starts unattended ones. Both
programs start Claude Code and Codex with the machine's own CLI configuration. The owner's accounts sit
behind one CLIProxyAPI hub. Theo's hub guide (upstream link above) points each machine's Claude Code and
Codex configuration at the hub, and every program that runs those CLIs then uses the hub with no extra
wiring. T3 Code keeps one Claude entry and one Codex entry, and the hub routes by model name.

The owner's eleven Claude launch profiles share one base URL and one key reference. They differ only in the
model and the small fast model. So the hub plus T3's model picker does what one T3 entry per profile would
do. Some parts stay manual in the guide: editing each client's files, the one-hour prompt cache that Claude
Code drops on a custom endpoint, the routed model names in T3, and checks that the setup still holds.

Two earlier attempts used the Agent Client Protocol (ACP): a T3 Code entry that runs `rune run`, and a
Multica runtime profile in the ACP family (Z-122). rune speaks no ACP, so neither works.

## Decision Drivers

- Use what the hub and the interfaces already do, and maintain only the rest
- One home for routes and key references: rune launch profiles
- A secret value never lands in a configuration file as a literal
- The interfaces keep their native drivers
- Removal is exact: rune removes what it wrote and nothing else

## Considered Options

1. **Theo's baseline first, rune second**: each machine's Claude Code and Codex configuration points at the
   hub, as in the guide. rune applies that client step from the launch profiles, sets the cache lifetime,
   writes the routed model names into T3, and checks the setup. Launch wrappers stay only for profiles that
   need arguments or middleware an interface cannot express.
2. **One launcher per profile**: rune registers one T3 entry and one Multica runtime per profile. With one
   base URL and routing by model name, the entries only repeat what the model picker does, and they clutter
   both interfaces.
3. **An ACP server in rune** (Z-122): rune answers ACP and drives the harness itself. T3's native Claude and
   Codex drivers already resume sessions, import history, run the model picker, and report usage. An ACP
   adapter would replace all of that with code rune maintains.
4. **The guide by hand**: the owner edits each machine. Literal keys end up in settings files, the cache
   lifetime gets forgotten, and drift goes unnoticed.

## Decision Outcome

Option 1.

`rune interface enable t3code` applies the client step when it is missing, sets the cache lifetime, and writes
the routed model names into T3's Claude entry. `rune interface enable multica` checks that Multica's runtimes
inherit the machine configuration. `rune interface status` checks the hub, the cache lifetime, session
affinity, Codex websockets, Theo's rule against Claude subscription models outside Claude Code, and stale ACP
entries.

Installing and running the hub, account logins, routing strategy, and dashboards stay with CLIProxyAPI's own
installer, login commands, and management panel. Installing skills into Multica (Z-123) is a provider concern
and stays out of this decision.

## Consequences

- rune edits two foreign files on each machine, Claude Code's settings and Codex's configuration, and reads
  T3 Code's settings. A format change in any of them breaks the step until rune follows it. Status shows it.
- T3 offers one Claude entry. Per-profile differences other than the model, such as the small fast model,
  apply machine-wide.
- The owner keeps the choice between hub-only clients and a second configuration directory that leaves the
  normal install untouched.
- The T3 Code ACP entry and the Multica ACP profile from the earlier attempts become stale findings that the
  owner removes.
