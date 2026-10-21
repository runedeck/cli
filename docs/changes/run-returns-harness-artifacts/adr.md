---
title: "A run reports the files the harness created and copies them where they survive"
description: "rune run lists the files an agy run left at the top level of brain/<conversation_id>/, copies them into --artifacts-dir or, on a clean run, under the rune state folder, and names them in the result; other harnesses report an empty list"
type: adr
category: architecture
tags:
    - cli
    - run
    - artifacts
status: proposed
created: 2026-10-21
updated: 2026-10-21
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
change: run-returns-harness-artifacts
---

# A run reports the files the harness created and copies them where they survive

## Context and Problem Statement

Agy writes a generated file under `brain/<conversation_id>/` in its data directory, not in the repository, and its JSON answer names the conversation and the text only. On 2026-10-21 a print-mode run drew an icon in one turn and the image sat in that folder with no mention in the result. A clean run sets the data directory to a temporary root the runner deletes at the end, so the file is gone before the caller reads the answer. The caller needs the paths in the result and, on a clean run, a copy that outlives the run.

## Considered Options

1. Read the conversation folder once after the reply, filter by top level, bookkeeping names, and the run's start time, copy on request or on a clean run, and name the files in the result.
2. Watch the folder during the run. A thread, and a file written after the last poll is missed.
3. Parse agy's event stream for the files its tools wrote. The stream names the tool calls, not the files they leave, and changes with agy.
4. Keep the clean root when it holds a file. The root then outlives the run with the token copy inside it.

## Decision Outcome

Option 1. The runner MUST list the files an agy run created at the top level of `brain/<conversation_id>/`, skipping `*.metadata.json`, `.system_generated/`, `.user_uploaded/`, `scratch/`, and files older than the run's start. `--artifacts-dir <DIR>` MUST copy them into DIR with a numeric suffix on a clash. A clean run without the flag MUST copy them under `$XDG_STATE_HOME/rune/artifacts/<conversation_id>/` before the clean state goes. The result MUST carry `artifacts`, and every other harness MUST report an empty list.

## Consequences

- The caller gets paths, and on a clean run the copies survive with no credential beside them.
- A file in a nested folder is not reported. When agy starts nesting generated files, the filter needs a new rule.
- The start-time filter reads modification times at second granularity. A file agy rewrites during the run is reported again.
- A failed copy ends the run with an error after the harness answered.
