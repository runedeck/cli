---
adr: docs/changes/dispatcher-records-every-hook-execution/adr.md
status: proposed
decisions: ["The hook dispatcher records every hook execution"]
---

# Dispatcher records every hook execution

## Why

The *[hook dispatcher][HOOKDISPATCHER]* records every *[hook][HOOK]* it runs as one line, a hook may add *[hook metrics][HOOKMETRICS]*, and `rune hook record` reads the *[hook record][HOOKRECORD]*.

A *[hook execution][HOOKEXECUTION]* leaves no trace of its own. The lint hook asks to deny a write or returns advice, and both reach only the agent and the harness transcript. The audit of 2026-10-16 had to parse 8 GB of Claude Code transcripts to learn that 247 of 305 repeat denials carried the same causes as the denial before, and that MD013 alone caused 214 denials. A 316-line Python script in the owner's dotfiles now does that parse. It reads one harness, after the fact, from a file format rune does not own. SpecStory transcripts drop the hook attachments, and Claude Code's OpenTelemetry export carries no hook output ([Claude Code monitoring][CC-MON]), so neither replaces the parse.

## What Changes

- `rune hook run` writes one hook record per hook it runs to a monthly JSONL file under the rune state dir, `hooks/record/YYYY-MM.jsonl`: harness, event, hook, session, tool-use id, file, duration, exit status, what the hook asked for, the *[dispatcher answer][DISPATCHERANSWER]*, failure, metrics. The write happens after the answer is encoded and never delays it.
- A hook's output gains an optional `metrics` object of identifiers and integers. The `lint-on-write` adapter fills it with rule counts and the causes of a denial.
- `rune hook record` lists the record with filters. `rune hook record stats` aggregates by rule, week, file, and hook. `rune hook record import --harness claude` backfills from the raw session mirrors under `~/Data/Sessions/claude`, or from `~/.claude/projects`.
- The record holds rule ids, counts, paths, and durations. It holds no finding text, misspelled word, prompt text, or file content, and no rune command transmits it or packs it into an artifact.
- The dotfiles `lint-stats` script and its bookmark `change/lint-stats` retire after one import.

## Capabilities

### New Capabilities

- `dispatcher-records-every-hook-execution`: the record the dispatcher writes, its fields and limits, and the metrics a hook may add.
- `owner-reads-the-hook-record`: the three read commands, `rune hook record`, `rune hook record stats`, and `rune hook record import`.

### Modified Capabilities

None. The hook's output keeps every field `one-dispatcher-runs-every-hook` names and gains one optional field the encoders ignore.

## Impact

- `src/cli/hook/run.rs` (the write after the answer), `src/cli/hook/encode.rs` (`HandlerResult.metrics`), `src/cli/hook/adapters.rs` (`lint_on_write_in` counts what it reports), new `src/cli/hook/record.rs`, `src/cli/hook/stats.rs`, `src/cli/hook/import_claude.rs`, `src/cli/mod.rs` (`HookAction::Record`), `tests/fixtures/root-help.txt`, `tests/hook_record_*.rs`, `docs/proofs/dispatcher-records-every-hook-execution/`.
- The specification sits on main. The code depends on `one-dispatcher-runs-every-hook` (`fable/hooks-1`, signed at `68fb6321`, still off main) and merges after it.
- dotfiles: `change/lint-stats` is dropped after the first import. The shell lint hook retires with `sd hook`, as the hooks change already plans.
- Records: `adr.md` in this change. Related: the hooks change's record, CLI-0041 (the state dir), DOT-0007.
- Origin: the drawing `obsidian://open?vault=Atlas&file=Library%2FCanvas%2FHooks%20Record%20Log`, snapshot under `docs/ideas/`. This change adds `scripts/sync-idea` to this repository, copied from the deck.
- Terms: the deck's ontology defines hook, hook dispatcher, hook execution, hook record, hook metrics, and dispatcher answer. cli reads a deployed copy of it, `ontology/rune.ttl`, written by `rune install` from the `.rune` key `ontology: deck` (cli change `install-deck-ontology`, on main), and the specs cite each term at first use.

[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
[HOOK]: ../../specs/glossary/hook.md
[HOOKMETRICS]: ../../specs/glossary/hook-metrics.md
[HOOKRECORD]: ../../specs/glossary/hook-record.md
[HOOKEXECUTION]: ../../specs/glossary/hook-execution.md
[DISPATCHERANSWER]: ../../specs/glossary/dispatcher-answer.md
[CC-MON]: https://code.claude.com/docs/en/monitoring-usage
