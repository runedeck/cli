---
title: "The hook dispatcher reads the agent policy before it starts a handler"
description: "A hook handler may name a policy entry, the dispatcher resolves it for the event's directory and session before it starts the handler, and a refused handler does not start"
type: adr
category: architecture
tags:
    - cli
    - hooks
    - policy
status: proposed
created: 2026-10-30
updated: 2026-10-30
author: "@N4M3Z"
project: cli
related:
    - "Hooks route through one dispatcher declared in the rune config"
    - "The hook dispatcher records every hook execution"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream:
    - "one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy"
change: policy-gates-hook-handlers
---

# The hook dispatcher reads the agent policy before it starts a handler

## Context and Problem Statement

The agent policy file grants dotted names per directory. Jev reads it itself, so only Jev obeys it. Every other handler runs wherever its event fires. The owner wants to turn any handler off in one place or for one session, and wants a cloned repository to be able to turn handlers off but never on. The question is which program reads the policy for a handler.

## Considered Options

1. Every handler reads the policy itself, as Jev does.
2. The dispatcher reads the policy for a handler that names a policy entry, and skips the handler when the policy refuses it.
3. Install reads the policy and leaves refused handlers out of the compiled plan.

## Decision Outcome

Option 2. A handler MAY carry `policy: <name>`. Before the dispatcher starts it, the dispatcher MUST resolve the name for the event's working directory and session id, and a refused handler MUST NOT start. The dispatcher log and the hook record MUST say `skipped by policy: <name> (<layer>)`. Rune carries one reader in Rust with the semantics of the Jev reader, plus a repository layer that may only take away and a session switch file.

Option 1 puts a copy of the reader in every handler, in every language, and the copies drift. Option 3 decides once per install, so it cannot see the event's directory or session.

### Consequences

- [+] Any handler can be turned off per directory, per repository, or per session without a change to the hook config.
- [+] `rune policy check` and `jev policy` print the same line for the same question.
- [-] Each event with a policy-named handler reads the policy file, the repository file, and the session file. The reads are small and local.
- [-] A legacy dispatcher part that covers a handler still runs its own command. The policy cannot stop that part.
- [-] Two readers exist until Jev calls `rune policy check`. A shared test list keeps them in step.
