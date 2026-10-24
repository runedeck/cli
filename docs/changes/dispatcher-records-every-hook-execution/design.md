# Design

The origin is the drawing `Library/Canvas/Hooks Record Log` in the owner's vault, snapshot under `docs/ideas/`. The audit behind it is in the runedeck workshop, `docs/specs/2026-10-16-hooks-record-log/plan.md`. This file holds what the code needs.

## Approach

The hook dispatcher writes the hook record, and a hook only adds hook metrics to its output. Every hook then leaves a line with its duration, exit status, what it asked for, and the dispatcher answer, and the lint adapter adds what it alone knows, the rules. The alternative where the lint adapter logs for itself covers one hook and leaves dcg, rtk, and the guards unmeasured. The status quo, a Python script over Claude transcripts, covers one harness after the fact and never sees a clean run.

## Runtime

```text
rune hook run --harness H --native-event N
        |
for hook in plan[H][event]:
    outcome = spawn and wait, or "budget" when none is left
    records.push(Record { v, ts, harness, event, native_event, hook,
                          session_id, tool_use_id, cwd, file, duration_ms,
                          exit_status, asked, failure, metrics })
        |
answer = the dispatcher answer for the event; every record of the execution gets it
        |
encode the answer for H as before, print it
        |
writer thread: create <state>/hooks/record/ if missing, open YYYY-MM.jsonl
with O_APPEND, write every record as one line, close    <- no fsync
main thread: join the writer for at most the reserve kept for the answer and record;
on failure or on a late writer, one stderr line names the path and the
records lost; exit with the answer's code either way
```

The month comes from the record's own UTC timestamp. The writer runs after the answer is on stdout, so the harness never waits on the disk beyond the part of the event budget the dispatcher keeps back for its own answer and record.

## Structure

- `src/cli/hook/record.rs`: `Record`, `path_for(ts)`, `write_all(records, deadline)`. Reads `state::dir()`, creates the directory, and bounds the writer thread.
- `src/cli/hook/encode.rs`: `HandlerResult` gains `metrics: Option<Metrics>`, validated on parse: identifier keys `[A-Za-z0-9._:/-]{1,64}`, values integers, lists of identifiers, or identifier-to-integer objects, at most 4096 UTF-8 bytes serialized. A failed validation drops the object and names the hook on stderr. Encoders never read it. The cap keeps a month of records under 4 MB at the audited rate of about 240 runs a week, even when every record reaches it.
- `src/cli/hook/run.rs`: one `Record` pushed per hook it runs or fails to start, the dispatcher answer filled in once the event is answered, all written together after the answer.
- `src/cli/hook/adapters.rs`: `lint_on_write_in` builds `metrics` (`rules`, `cause`) while it builds the deny and advice strings. Misspelled words are file content and stay out of the record. The line parser moves to `src/cli/hook/findings.rs` so the import uses the same four shapes: rumdl `path:line:col: [MDnnn] message`, Vale `path:line:col severity Check: message`, Vale's line format `path:line:col:Check:message`, typos ``error: `word` should be ...``.
- `src/cli/hook/stats.rs`: `load(filters)`, `list`, `stats`, the table and JSON printers.
- `src/cli/hook/import_claude.rs`: walk, prefilter by substring, read from the stored offset, map tool-use id to file path within one transcript, parse attachments, dedupe on harness, session, tool-use id, and hook against live and imported records, write. The checkpoint `<state>/hooks/record/import-claude.json` keeps, per file, the offset after the last complete line and the tool-use ids read so far.
- `src/cli/mod.rs`: `HookAction::Record { action: Option<RecordAction> }` with `Stats` and `Import`.

## Data

- Record: `{"v":1,"ts":"2026-10-16T00:10:16.912Z","harness":"claude","event":"tool.after","native_event":"PostToolUse","hook":"lint-on-write","session_id":"…","tool_use_id":"toolu_…","cwd":"/…","file":"/…/probe.md","duration_ms":181,"exit_status":0,"asked":"deny","answer":"deny","failure":null,"metrics":{"rules":{"Core.NoEmDash":1,"STE.Semicolons":1,"typos":1},"cause":["Core.NoEmDash","typos"]}}`.
- An imported record carries `"source":"import-claude"` and null `duration_ms`. The identity of a run, for the import's duplicate check, is harness, session, tool-use id, and hook.
- Stats rule row: `rule, findings, cause_runs, sole_cause_runs, advice_runs`.

## Risks

- The write sits after the answer, inside the part of the event budget the dispatcher keeps back for itself. A disk that blocks loses that event's records and costs one stderr line. The harness never waits on it.
- The code depends on `one-dispatcher-runs-every-hook`, signed at `68fb6321` and not yet on main, which still has two open task groups. The specification sits on main. The code waits for that change.
- Claude Code owns the transcript format the import reads. The import is a backfill, not the record's source, so a format change costs the backfill and nothing else.
- Paths in the record may name AMBER vault notes. The record is local, and the last requirement of `owner-reads-the-hook-record` keeps it out of every artifact rune produces.
- The proof runner sets `RUNE_STATE_DIR` to a fresh directory for each scene and clears `XDG_STATE_HOME`. A scene that names `RUNE_STATE_DIR=…` itself keeps its fixture, because the scene's variables come after the runner's. Without that default, a scene running `rune hook record` copied the owner's live record into `proof.txt` and `proof.cast`.
- `tests/hook_record_local.rs` runs a release and a proof with a populated record and scans the source for readers of the record directory outside the hook module. `rune bench report` and `rune sign` are covered by that scan only, because neither has a fixture that produces an artifact without a bench suite or a key.

## Coverage

The specs state behavior. The commands live here, in the tasks, and in the proposal.

| capability | requirement | check or open gap |
|---|---|---|
| dispatcher-records-every-hook-execution | The Hook Dispatcher Records Every Hook Execution | `tests/hook_record_write.rs`: two hooks in order, budget, month boundary, empty state dir |
| dispatcher-records-every-hook-execution | A Hook Record Carries Fixed Fields | `tests/hook_record_write.rs`: a denied lint write names hook, event, asked, and answer, the budget case names failure budget |
| dispatcher-records-every-hook-execution | Recording Never Delays Or Changes The Answer | `tests/hook_record_write.rs`: read-only directory and a blocked writer, answer and exit code compared byte for byte |
| dispatcher-records-every-hook-execution | The Record Holds No Text | `tests/hook_record_metrics.rs`: a quoted sentence absent from the file |
| dispatcher-records-every-hook-execution | A Hook May Return Counted Metrics | `tests/hook_record_metrics.rs`: conforming, text in value and key, 4096 and 4097 bytes |
| dispatcher-records-every-hook-execution | The Lint Hook Reports Its Rules | `tests/hook_record_metrics.rs` on the em-dash-semicolon-typo fixture and the clean fixture |
| owner-reads-the-hook-record | The Record Lists On Request | `rune hook record [--since W] [--until W] [--harness H] [--hook H] [--rule S] [--file S] [--asked A] [--answer A] [--json]`, `tests/hook_record_stats.rs` |
| owner-reads-the-hook-record | Stats Aggregate The Record | `rune hook record stats [filters] [--json]`, `tests/hook_record_stats.rs`: sole cause row, JSON equals table |
| owner-reads-the-hook-record | Stats Define Denial, Advice, Repeats, And Top | `rune hook record stats --top N`, `tests/hook_record_stats.rs`: repeats, tie |
| owner-reads-the-hook-record | Import Backfills From Raw Claude Sessions | `rune hook record import --harness claude [DIR]`, default `~/Data/Sessions/claude` else `~/.claude/projects`, `tests/hook_record_import.rs` |
| owner-reads-the-hook-record | Import Keeps A Checkpoint | `<state>/hooks/record/import-claude.json`, `tests/hook_record_import.rs`: grown transcript, earlier tool use, half line |
| owner-reads-the-hook-record | The Record Stays On The Machine | one test per artifact-writing command in `tests/hook_record_local.rs`: release, bench report, proof run, sign |

## Terms and ontology

The owner chose one ontology, the deck's `ontology/rune.ttl`, read by cli through a configured path (2026-10-16). The deck defines the terms this change cites, after the owner's review of the whole ontology on 2026-10-16 (deck main `861ae76c`, owner-signed, 79 labeled terms): hook (a script bound to harness events, run by the hook dispatcher), hook dispatcher, hook execution, hook record, hook metrics, and dispatcher answer (allow, deny, or stop the turn, with a reason). The specs, the proposal, and `adr.md` set each first use in italics and cite it as `*term* [TERM]`, with `[TERM]: https://runedeck.ai/ns#Term` at the end of the file. Dropped by that review: handler (write hook, or the hook's process), response reserve (said in a sentence), hook effect. Plain nouns in their dictionary sense carry no tag: state dir, tool-use id, session, event, exit status.

How cli reaches the ontology: the owner chose that every consumer reads a deployed copy. The cli change `install-deck-ontology` (bookmark `fable/install-deck-ontology`) gives `.rune` a top-level `ontology: <source>` key, and `rune install` deploys `<source>/ontology/rune.ttl` to `<consumer>/ontology/rune.ttl` with a provenance sidecar and a fingerprint, where the term reader in `rune-docs/src/spec/terms.rs` already looks. That change is on main (`f721ba57425f`), this repository's `.rune` names `ontology: deck`, and `ontology/rune.ttl` at deck `861ae76c` is committed here. The deployed copy is a tracked file, so a jj workspace reads it like the main checkout, and the term lint resolves every citation in this change against it.

## Review dispositions

Astra on the specification and the record (`gpt-6-astra`, clean state, 12 hits, 2026-10-16):

| Hit | Disposition |
|---|---|
| 1 misspelled words are file content | fixed: `words` removed from the metrics, the record, and the stats. Typos count under the id `typos` |
| 2 free-form metrics strings can carry text | fixed: metrics values are integers, lists of identifiers, or identifier-to-integer objects, keys and identifiers match `[A-Za-z0-9._:/-]{1,64}`, a failed validation drops the object. Scenario with injected text added |
| 3 "No … MUST include" is weak and names only file names | fixed: "rune MUST NOT transmit record data or include it … under any file name or embedded in another file", proof scenario added |
| 4 a synchronous write can still eat the harness deadline | fixed: records are collected and written after the answer by a writer thread the dispatcher joins for at most the reserve it keeps for its own answer, late or failed writes lose the records. Blocked-writer scenario added |
| 5 one key for several attachments, live and imported overlap undefined | fixed: identity is harness, session, tool-use id, and hook, live records carry `tool_use_id`, scenario for a live run already recorded |
| 6 an attachment can refer to a tool use before the offset | fixed: the checkpoint keeps the tool-use ids per file and advances only past complete lines, two scenarios added |
| 7 nothing creates `hooks/record/` | fixed: the dispatcher creates the directory, empty-state-dir scenario added |
| 8 advice, the compared denial, and `--top` undefined | fixed: new requirement "Stats Define Advice, Repeats, And Top", tie scenario added |
| 9 date boundaries, timezone, filter combination unspecified | fixed: inclusive UTC periods, all filters must match, scenarios for combined filters and the empty record |
| 10 the denial policy behind the lint scenario is unstated, the clean-file outcome has no MUST | fixed: the scenario's WHEN names the Vale levels and the typos policy, the requirement states the clean-file MUST |
| 11 4096 bytes has no stated reason, boundary untested | fixed: the cap is tied to the audited rate (about 240 runs a week, under 4 MB a month at the cap), scenario tests 4096 and 4097 |
| 12 semicolon in the record description, passive MUSTs, "writes down", a triad, state-shaped scenario titles | fixed throughout, every scenario title names an event |

Astra on the code (`gpt-6-astra`, clean state, 12 hits, 2026-10-17):

| Hit | Disposition |
|---|---|
| 1 the writer got a fresh reserve, not what is left of the budget, and a 50 ms floor | fixed in part: the floor is gone, and the writer waits for what the answer left of the reserve. The reserve is not reduced by a hook's overrun, because the reserve is the dispatcher's own allowance and a hook that timed out at the edge of its budget must not empty it: the budget-spent proof scene showed every record lost under that reading. The answer is printed by `run` after `dispatch` returns, after the bounded wait. The harness reads a hook's stdout at exit, so the order on the pipe changes nothing |
| 2 metrics validation runs inside the budget charged to later hooks, deep nesting fails the result | disputed: validation serializes at most 4 KB once per hook, microseconds against a 500 ms timeout. A result that does not parse as one JSON object is the existing contract of `one-dispatcher-runs-every-hook`, metrics or not |
| 3 `thread::spawn` panics when no thread can start | fixed: `Builder::spawn`, a failure is the one stderr line naming the records lost |
| 4 appends can interleave across processes, a torn tail poisons `load` | fixed: writers and the import take a lock file in the record directory, each monthly file is written in one call per execution, and `load` skips a line that is not a record with one stderr note |
| 5 duplicate check, append, and checkpoint are not one operation | fixed: the import holds the directory lock across the check and the append, and the checkpoint is written to a temp file and renamed |
| 6 checkpoints keyed on a displayed, relative path | fixed: the canonical path keys the checkpoint. A file that shrinks below its offset is read from the start |
| 7 `rune hook run` accepted as lint provenance | fixed: only the legacy shell hook and `lint-on-write` mark an attachment. The dispatcher's own outcomes are in the live record |
| 8 cause counts ignore denial, advice counts "not deny" | fixed: a cause counts only in a run that asked to deny, advice only in a run that asked for allow |
| 9 rows rank by findings, so a tie on caused denials is not what `--top` cuts | fixed: rows rank by denials caused, then findings, then name. The proof scene is a real three-way tie |
| 10 filters before `stats` are discarded | fixed: both filter sets merge, the one after `stats` wins per field |
| 11 the proof test ran `spec list` | fixed: `rune proof run` on the `filled-good` fixture with a populated state dir. Bench and sign stay covered by the source scan only, a stated limit |
| 12 a closure clippy would reject | applied, though the check had passed |

Grok on the code (`grok-4.7` through the cliproxy route, clean state, 11 hits, 2026-10-17). Five restate Astra's 1, 4, 9, 8, and 11 and took their fixes. The rest:

| Hit | Disposition |
|---|---|
| 4b a rule named only in `cause` gets no row | fixed: a cause the rules object does not name still gets its row and counts |
| 5 imported advice keeps a cause list | fixed: an attachment that advised has its causes cleared, as the adapter does |
| 6 a missing session or tool-use id collapses into one identity | fixed: a record without both has no identity and is never a duplicate |
| 7 an unresolved state dir loses records with no line | fixed: the runner keeps the resolution error and reports it once per event that had records |
| 8 `walk` follows symlink cycles | fixed: symlinks are not followed |
| 10 the clean-file test accepted any rules object and skipped without rumdl | fixed: rules and cause must both be empty, with or without the linters |
| 11 a week whose Sunday is out of range panics | fixed: the same "not an ISO week" error |
