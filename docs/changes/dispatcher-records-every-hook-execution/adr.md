---
title: "The hook dispatcher records every hook execution"
description: "The hook dispatcher writes one hook record per hook it runs into a monthly file under the state dir after the dispatcher answer is encoded, a hook may add counted metrics, and rune hook record reads the record"
type: adr
category: architecture
tags:
    - cli
    - hooks
    - lint
    - record
status: proposed
created: 2026-10-16
updated: 2026-10-16
author: "@N4M3Z"
project: cli
related:
    - "Hooks route through one dispatcher declared in the rune config"
    - "CLI-0041 Signing Queue"
    - "DOT-0007 Vendor Binaries on PATH and rune run for Automation"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1@claude", "gpt-6-astra@codex"]
informed: []
upstream:
    - "obsidian://open?vault=Atlas&file=Library%2FCanvas%2FHooks%20Record%20Log"
    - "one-dispatcher-runs-every-hook#handlers-run-in-order-and-fail-by-policy"
    - "one-dispatcher-runs-every-hook#the-dispatcher-normalizes-and-answers"
change: dispatcher-records-every-hook-execution
---

# The hook dispatcher records every hook execution

## Context and Problem Statement

A *[hook][HOOK]* asks to deny a write, returns advice, or passes, and nothing remembers which. The owner asked on 2026-10-16 whether Vale keeps statistics of what triggered. It does not, and neither does the [hook][HOOK]. The audit that answered the question parsed 8 GB of Claude Code transcripts and found that 247 of 305 repeat denials carried the same causes as the denial before them, and that MD013 alone caused 214 denials. That parse now lives as a Python script in the owner's dotfiles. It reads one harness, from a format rune does not own, and never sees a clean run. SpecStory transcripts drop the [hook][HOOK] attachments. Claude Code's OpenTelemetry export carries no [hook][HOOK] output and the `claude_code.hook` span exists only under a beta tracing mode ([Claude Code monitoring][CC-MON]). The question is what writes the record, and where.

## Considered Options

1. Keep the dotfiles script that parses Claude transcripts after the fact.
2. The lint adapter logs its own runs.
3. The *[hook dispatcher][HOOKDISPATCHER]* records every *[hook execution][HOOKEXECUTION]*, and a [hook][HOOK] may add *[hook metrics][HOOKMETRICS]* to its output.

## Decision Outcome

Option 3. `rune hook run` MUST write one *[hook record][HOOKRECORD]* per [hook][HOOK] it runs or fails to start to `hooks/record/YYYY-MM.jsonl` under the rune state dir, with harness, event, [hook][HOOK], session, tool-use id, file, duration, exit status, what the [hook][HOOK] asked for, the *[dispatcher answer][DISPATCHERANSWER]*, failure, and metrics. It MUST write after the answer is encoded, within the part of the event budget it keeps back for its own answer and record. A write that fails or runs late loses the records and prints one stderr line, and the answer stays as encoded. A record MUST hold identifiers, counts, paths, and durations, never a finding's text, a misspelled word, a prompt, or file content. A [hook][HOOK]'s output MAY carry `metrics` of identifiers and integers, at most 4096 bytes, copied unchanged. `lint-on-write` MUST report `rules` and `cause`. `rune hook record` lists, `rune hook record stats` aggregates, and `rune hook record import --harness claude` backfills from the raw session mirrors under `~/Data/Sessions/claude`. rune MUST NOT transmit record data or include it in any artifact it produces.

Option 1 keeps a parser for a format Claude Code can change, covers no other harness, and cannot count clean runs. Option 2 measures one [hook][HOOK] and leaves dcg, rtk, and the guards without duration or answer.

### Consequences

- [+] One command answers which rules deny, where, how often, and whether a retune changed anything, for every [hook][HOOK] and every harness the dispatcher serves.
- [+] Clean runs are counted, so a denial rate per write exists for the first time.
- [+] The lesson of the audit is built in: a rule among the causes and a rule as the sole cause are two columns.
- [-] Every event pays one file write after its answer, bounded by the reserve the dispatcher keeps for itself. A slow disk loses records and never delays the harness.
- [-] The [hook][HOOK]'s output schema gains a field. [Hook][HOOK] authors who return metrics need the identifier rule, the integer values, and the cap.
- [-] Misspelled words stay out of the record because they are file content. Diagnosing a language mismatch, such as Czech notes under an English dictionary, falls back to the file path and the typos count.
- [-] The record names files, including vault notes the owner keeps local. It stays under the state dir and out of every release, bench, proof, and signing artifact.
- [-] The backfill depends on Claude Code's transcript format. A format change breaks the backfill and leaves the record intact.
- [-] A harness the dispatcher does not register leaves no record until its adapter exists.

[HOOK]: ../../specs/glossary/hook.md
[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
[HOOKEXECUTION]: ../../specs/glossary/hook-execution.md
[HOOKMETRICS]: ../../specs/glossary/hook-metrics.md
[HOOKRECORD]: ../../specs/glossary/hook-record.md
[DISPATCHERANSWER]: ../../specs/glossary/dispatcher-answer.md
[CC-MON]: https://code.claude.com/docs/en/monitoring-usage
