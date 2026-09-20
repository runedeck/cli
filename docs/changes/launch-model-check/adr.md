---
title: "Model routes are checked against the endpoint on demand"
description: "A --check mode on launch and run asks each base URL in the plan for its model list with the plan's own credential, and never spawns; the Kimi K3 route is built in with a stated context assumption."
type: adr
category: architecture
tags:
    - cli
    - launch
    - models
status: proposed
created: 2026-09-20
updated: 2026-09-20
author: "@N4M3Z"
project: cli
related:
    - "CLI-0031 Launch Profiles"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: launch-model-check
---

# Model routes are checked against the endpoint on demand

## Context and Problem Statement

A profile's model route is a string the CLI passes on. Whether the endpoint serves that id under the profile's credential is unknown until the harness fails. The owner registered Kimi K3 on the local proxy and asked for a way to see, from `rune`, that `kimi-k3` is served before an interactive session or a supervised run starts. The route table also needs the model itself.

## Considered Options

1. Check on every launch: resolve, call the endpoint, refuse on a miss, then spawn.
2. A `--check` mode on `launch` and `run`: resolve, call the endpoint, report, exit. No spawn.
3. A `rune doctor` section that reads every profile from config and checks each model id.

## Decision Outcome

Option 2. `rune launch --check` and `rune run --check` MUST resolve the same plan the command would execute, MUST ask each base URL for `/v1/models` with the credential the plan sends there, and MUST report each model id the plan sends as served or missing. They MUST NOT spawn the tool or send a prompt. Exit codes MUST be 0 for all served, 1 for a missing id, and 2 for an endpoint that fails or refuses. No credential value MUST appear in the report.

The route table gains `kimi` with id `kimi-k3` and context 262144. The owner's Kimi plan shows the Standard context tier, with Extra Long (1M) locked to a higher tier, and the proxy names a `kimi-k3-256k` route beside `kimi-k3` ([Kimi K3 model card][CARD], owner screenshot of 2026-09-20). The standard limit is not published by the proxy. 262144 is an assumption: the highest limit the route names imply for the Standard tier. A config `models.kimi` overrides it.

Option 1 puts a network round trip and a new failure in front of every interactive start, including offline ones. Option 3 cannot know which credential a profile sends, because the credential is resolved per profile from the environment file, and it would duplicate the resolver.

## Consequences

- A wrong route is visible in one command before any harness starts. Automation runs `rune run <tool> --check` before a long job.
- "Served" means the endpoint lists the id for this credential. It does not prove a completion succeeds.
- A proxy without `/v1/models` makes the check exit 2 for every profile that points at it. The launch itself is unchanged.
- The Kimi context is a recorded assumption. When the proxy or Moonshot publishes the standard limit, the built-in changes and the record gets an audit line.

[CARD]: https://huggingface.co/moonshotai/Kimi-K3
