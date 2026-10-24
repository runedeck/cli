---
type: proof
change: dispatcher-records-every-hook-execution
head: f95d10ae6915d1a1afe98c42da0a74bea62d7085
recorded: 2026-10-24
transcript: 60ddb4c6156a3d845e866c3c750f42b52818f802beb68bea35644b98e5229b75
scenes:
- scenario: dispatcher-records-every-hook-execution#the-hook-dispatcher-records-every-hook-execution/two-hooks-run-on-one-event
  kind: check
- scenario: dispatcher-records-every-hook-execution#the-hook-dispatcher-records-every-hook-execution/executions-cross-a-month-boundary
  kind: check
- scenario: dispatcher-records-every-hook-execution#the-hook-dispatcher-records-every-hook-execution/dispatcher-runs-on-an-empty-state-dir
  kind: check
- scenario: dispatcher-records-every-hook-execution#a-hook-record-carries-fixed-fields/hook-asks-to-deny-and-the-event-is-denied
  kind: check
- scenario: dispatcher-records-every-hook-execution#a-hook-record-carries-fixed-fields/budget-runs-out-before-a-hook
  kind: check
- scenario: dispatcher-records-every-hook-execution#recording-never-delays-or-changes-the-answer/record-directory-is-read-only
  kind: check
- scenario: dispatcher-records-every-hook-execution#recording-never-delays-or-changes-the-answer/disk-blocks-the-writer
  kind: check
- scenario: dispatcher-records-every-hook-execution#the-record-holds-no-text/vale-quotes-a-sentence
  kind: check
- scenario: dispatcher-records-every-hook-execution#a-hook-may-return-counted-metrics/hook-returns-conforming-metrics
  kind: check
- scenario: dispatcher-records-every-hook-execution#a-hook-may-return-counted-metrics/hook-returns-text-in-metrics
  kind: check
- scenario: dispatcher-records-every-hook-execution#a-hook-may-return-counted-metrics/metrics-sit-at-the-cap-and-one-byte-above
  kind: check
- scenario: dispatcher-records-every-hook-execution#the-lint-hook-reports-its-rules/file-holds-one-em-dash-one-semicolon-one-misspelled-word
  kind: check
- scenario: dispatcher-records-every-hook-execution#the-lint-hook-reports-its-rules/file-has-no-finding
  kind: check
- scenario: owner-reads-the-hook-record#the-record-lists-on-request/filter-selects-one-hook-from-one-week-on
  kind: check
- scenario: owner-reads-the-hook-record#the-record-lists-on-request/filters-combine
  kind: check
- scenario: owner-reads-the-hook-record#the-record-lists-on-request/record-is-empty
  kind: check
- scenario: owner-reads-the-hook-record#stats-aggregate-the-record/rule-is-among-the-causes-and-the-sole-cause
  kind: check
- scenario: owner-reads-the-hook-record#stats-aggregate-the-record/json-matches-the-table
  kind: check
- scenario: owner-reads-the-hook-record#stats-define-denial-advice-repeats-and-top/session-is-denied-three-times-on-one-file
  kind: check
- scenario: owner-reads-the-hook-record#stats-define-denial-advice-repeats-and-top/top-cuts-a-tie
  kind: check
- scenario: owner-reads-the-hook-record#import-backfills-from-raw-claude-sessions/import-reads-two-transcripts-twice
  kind: check
- scenario: owner-reads-the-hook-record#import-backfills-from-raw-claude-sessions/live-record-already-holds-the-tool-use
  kind: check
- scenario: owner-reads-the-hook-record#import-keeps-a-checkpoint/transcript-grows
  kind: check
- scenario: owner-reads-the-hook-record#import-keeps-a-checkpoint/attachment-refers-to-an-earlier-tool-use
  kind: check
- scenario: owner-reads-the-hook-record#import-keeps-a-checkpoint/transcript-ends-mid-line
  kind: check
- scenario: owner-reads-the-hook-record#the-record-stays-on-the-machine/release-runs-with-a-populated-record
  kind: check
- scenario: owner-reads-the-hook-record#the-record-stays-on-the-machine/proof-runs-with-a-populated-record
  kind: check
- scenario: owner-reads-the-hook-record#the-record-stays-on-the-machine/proof-scene-reads-the-record
  kind: check
---

# Proof: dispatcher-records-every-hook-execution

One scene per scenario of the two delta specifications, in closure order. The hook scenes run the built `rune` against `tests/fixtures/hooks/record-home` (five shell hooks: one that passes, one that denies with counted metrics and a reason that quotes a sentence, one that returns text in its metrics, and two whose metrics sit at 4096 and 4097 bytes), `record-budget` (one slow hook that spends the budget), `record-lines` (eight records over three weeks), and `record-store` (two Claude transcripts). The lint hook scenes need rumdl, typos, and Vale with the owner's global prose layer, so they are recorded on the owner's machine.

The scenario `disk-blocks-the-writer` stays unproven here: no scene can make a device stall. The unit test `a_writer_past_the_deadline_loses_the_records_and_returns_in_time` in `src/cli/hook/record/tests.rs` injects a writer that sleeps past the deadline and shows the dispatcher returns in time with one stderr line.

## dispatcher-records-every-hook-execution#the-hook-dispatcher-records-every-hook-execution/two-hooks-run-on-one-event

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
$ python3 -
< import json, glob
< rows = [json.loads(l) for f in sorted(glob.glob("state/hooks/record/*.jsonl")) for l in open(f)]
< print([r["hook"] for r in rows])
['pass', 'lint', 'textual', 'cap', 'over']
```

## dispatcher-records-every-hook-execution#the-hook-dispatcher-records-every-hook-execution/executions-cross-a-month-boundary

```console
$# Two transcripts whose hook outcomes fall in September and October land in two monthly files.
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-store" store'
$ RUNE_STATE_DIR=state rune hook record import --harness claude store
read 2 transcript(s) under store: 4 record(s) added, 0 already held, into state/hooks/record
$ sh -c 'ls state/hooks/record/*.jsonl'
state/hooks/record/2026-09.jsonl
state/hooks/record/2026-10.jsonl
```

## dispatcher-records-every-hook-execution#the-hook-dispatcher-records-every-hook-execution/dispatcher-runs-on-an-empty-state-dir

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'test ! -e state/hooks/record && echo "no record directory"'
no record directory
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
$ sh -c 'ls state/hooks/record | sed "s/[0-9-]*\.jsonl/YYYY-MM.jsonl/"'
YYYY-MM.jsonl
```

## dispatcher-records-every-hook-execution#a-hook-record-carries-fixed-fields/hook-asks-to-deny-and-the-event-is-denied

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
$ python3 -
< import json, glob
< rows = [json.loads(l) for f in sorted(glob.glob("state/hooks/record/*.jsonl")) for l in open(f)]
< lint = [r for r in rows if r["hook"] == "lint"][0]
< print(lint["harness"], lint["event"], lint["native_event"], lint["session_id"], lint["tool_use_id"], lint["file"])
< print("asked:", lint["asked"], "answer:", lint["answer"], "exit:", lint["exit_status"], "failure:", lint["failure"])
< print("fields:", sorted(lint))
claude tool.after PostToolUse s1 toolu_9 a.md
asked: deny answer: deny exit: 0 failure: None
fields: ['answer', 'asked', 'cwd', 'duration_ms', 'event', 'exit_status', 'failure', 'file', 'harness', 'hook', 'metrics', 'native_event', 'session_id', 'tool_use_id', 'ts', 'v']
```

## dispatcher-records-every-hook-execution#a-hook-record-carries-fixed-fields/budget-runs-out-before-a-hook

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-budget" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
rune hook: slow failed: timed out after [..] ms (warn, proceeding)
rune hook: lint failed: the 300 ms budget of PostToolUse was spent before it started (warn, proceeding)
$ python3 -
< import json, glob
< rows = [json.loads(l) for f in sorted(glob.glob("state/hooks/record/*.jsonl")) for l in open(f)]
< print([(r["hook"], r["failure"], r["duration_ms"] if r["hook"] == "lint" else "-", r["answer"]) for r in rows])
[('slow', 'timeout', '-', 'allow'), ('lint', 'budget', 0, 'allow')]
```

## dispatcher-records-every-hook-execution#recording-never-delays-or-changes-the-answer/record-directory-is-read-only

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ sh -c 'mkdir -p state/hooks/record && chmod 555 state/hooks/record'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
rune hook: could not write 5 hook record(s) under state/hooks/record: [..]
$ sh -c 'chmod 755 state/hooks/record; ls state/hooks/record | wc -l | tr -d " "'
0
```

## dispatcher-records-every-hook-execution#recording-never-delays-or-changes-the-answer/disk-blocks-the-writer

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$# the month's record file is a FIFO with no reader, so a write to it never completes
$ sh -c 'mkdir -p state/hooks/record && mkfifo "state/hooks/record/$(date -u +%Y-%m).jsonl"'
$ sh -c 'start=$(date +%s); RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse; test $(( $(date +%s) - start )) -lt 3 && echo answered in under 3 s'
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
rune hook: the hook record writer did not finish within [..] ms; 5 record(s) lost under state/hooks/record
answered in under 3 s
```

## dispatcher-records-every-hook-execution#the-record-holds-no-text/vale-quotes-a-sentence

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
$ sh -c 'grep -c "quick brown fox" state/hooks/record/*.jsonl || echo "no sentence in the record"'
0
no sentence in the record
$ python3 -
< import json, glob
< rows = [json.loads(l) for f in sorted(glob.glob("state/hooks/record/*.jsonl")) for l in open(f)]
< print([r for r in rows if r["hook"] == "lint"][0]["metrics"])
{'cause': ['Core.NoEmDash'], 'rules': {'Core.NoEmDash': 1, 'STE.Semicolons': 1}}
```

## dispatcher-records-every-hook-execution#a-hook-may-return-counted-metrics/hook-returns-conforming-metrics

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
$ python3 -
< import json, glob
< rows = [json.loads(l) for f in sorted(glob.glob("state/hooks/record/*.jsonl")) for l in open(f)]
< print([r for r in rows if r["hook"] == "lint"][0]["metrics"])
{'cause': ['Core.NoEmDash'], 'rules': {'Core.NoEmDash': 1, 'STE.Semicolons': 1}}
```

## dispatcher-records-every-hook-execution#a-hook-may-return-counted-metrics/hook-returns-text-in-metrics

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
$ python3 -
< import json, glob
< rows = [json.loads(l) for f in sorted(glob.glob("state/hooks/record/*.jsonl")) for l in open(f)]
< textual = [r for r in rows if r["hook"] == "textual"][0]
< print("metrics:", textual["metrics"], "asked:", textual["asked"])
metrics: None asked: allow
$ sh -c 'grep -c "quick brown fox" state/hooks/record/*.jsonl || true'
0
```

## dispatcher-records-every-hook-execution#a-hook-may-return-counted-metrics/metrics-sit-at-the-cap-and-one-byte-above

```console
$ sh -c 'HOME="$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-home" RUNE_STATE_DIR=state rune hook list --write >/dev/null 2>&1'
$ RUNE_STATE_DIR=state rune hook run --harness claude --native-event PostToolUse
< {"hook_event_name":"PostToolUse","session_id":"s1","cwd":".","tool_name":"Write","tool_use_id":"toolu_9","tool_input":{"file_path":"a.md"},"tool_response":{}}
{"decision":"block","reason":"a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"the quick brown fox\" is quoted."}
rune hook: textual returned metrics the record refuses: value of `note` is not an integer, a list, or an object; dropped
rune hook: over returned metrics the record refuses: metrics is 4097 bytes; the cap is 4096; dropped
$ python3 -
< import json, glob
< rows = [json.loads(l) for f in sorted(glob.glob("state/hooks/record/*.jsonl")) for l in open(f)]
< by = {r["hook"]: r for r in rows}
< print("cap:", len(json.dumps(by["cap"]["metrics"], separators=(",", ":"))), "over:", by["over"]["metrics"])
cap: 4096 over: None
```

## dispatcher-records-every-hook-execution#the-lint-hook-reports-its-rules/file-holds-one-em-dash-one-semicolon-one-misspelled-word

```console
$# The real lint hook over a real file; rumdl, typos, and Vale run under the global prose layer.
$ sh -c 'printf "# Probe\n\nThis line carries an em dash — one semicolon; and the word refernce.\n" > a.md'
$ sh -c 'printf "%s" "{\"v\":1,\"harness\":\"claude\",\"event\":\"tool.after\",\"native_event\":\"PostToolUse\",\"cwd\":\".\",\"tool\":{\"name\":\"Write\",\"id\":\"toolu_9\",\"input\":{\"file_path\":\"a.md\"}}}" | rune hook adapter lint-on-write > result.json'
$ python3 -
< import json
< r = json.load(open("result.json"))
< print("effect:", r["effect"], "metrics:", json.dumps(r["metrics"], sort_keys=True))
< print("reason names the sentence:", "refernce" in r["reason"], "metrics name it:", "refernce" in json.dumps(r["metrics"]))
effect: deny metrics: {"cause": ["Core.NoEmDash", "typos"], "rules": {"Core.NoEmDash": 1, "STE.Semicolons": 1, "typos": 1}}
reason names the sentence: True metrics name it: False
```

## dispatcher-records-every-hook-execution#the-lint-hook-reports-its-rules/file-has-no-finding

```console
$ sh -c 'printf "# Clean\n\nOne plain sentence.\n" > clean.md'
$ sh -c 'printf "%s" "{\"v\":1,\"harness\":\"claude\",\"event\":\"tool.after\",\"native_event\":\"PostToolUse\",\"cwd\":\".\",\"tool\":{\"name\":\"Write\",\"id\":\"toolu_9\",\"input\":{\"file_path\":\"clean.md\"}}}" | rune hook adapter lint-on-write'
{"v":1,"effect":"pass","reason":null,"input":null,"context":[],"metrics":{"cause":[],"rules":{}}}
```

## owner-reads-the-hook-record#the-record-lists-on-request/filter-selects-one-hook-from-one-week-on

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$ RUNE_STATE_DIR=state rune hook record --hook lint-on-write --since 2026-W42
2026-10-12T10:00:00.000Z claude  PostToolUse  lint-on-write  allow allow    10 /w/b.md
2026-10-14T10:00:00.000Z claude  PostToolUse  lint-on-write  deny  deny     10 /w/c.md  [MD036]
2026-10-15T23:30:00.000Z claude  PostToolUse  lint-on-write  deny  deny     10 /w/d.md  [Core.NoEmDash MD013]
2026-10-16T10:00:00.000Z claude  PostToolUse  lint-on-write  allow allow    10 /w/d.md
```

## owner-reads-the-hook-record#the-record-lists-on-request/filters-combine

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$ RUNE_STATE_DIR=state rune hook record --rule MD013 --asked deny --until 2026-10-15 --json
{"v":1,"ts":"2026-10-05T10:00:00.000Z",[..]"tool_use_id":"toolu_a1",[..]"asked":"deny",[..]}
{"v":1,"ts":"2026-10-06T10:00:00.000Z",[..]"tool_use_id":"toolu_a2",[..]"asked":"deny",[..]}
{"v":1,"ts":"2026-10-07T10:00:00.000Z",[..]"tool_use_id":"toolu_a3",[..]"asked":"deny",[..]}
{"v":1,"ts":"2026-10-15T23:30:00.000Z",[..]"tool_use_id":"toolu_d1",[..]"asked":"deny",[..]}
```

## owner-reads-the-hook-record#the-record-lists-on-request/record-is-empty

```console
$ RUNE_STATE_DIR=state rune hook record
$ sh -c 'RUNE_STATE_DIR=state rune hook record --json | wc -c | tr -d " "'
0
```

## owner-reads-the-hook-record#stats-aggregate-the-record/rule-is-among-the-causes-and-the-sole-cause

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$ sh -c 'RUNE_STATE_DIR=state rune hook record stats | grep -E "^(rule|MD013|STE.Semicolons)\b"'
rule	findings	cause	sole cause	advice
MD013	8	4	2	0
STE.Semicolons	7	0	0	2
```

## owner-reads-the-hook-record#stats-aggregate-the-record/json-matches-the-table

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$ sh -c 'RUNE_STATE_DIR=state rune hook record stats > table.txt && RUNE_STATE_DIR=state rune hook record stats --json > stats.json'
$ python3 -
< import json
< table = open("table.txt").read(); s = json.load(open("stats.json"))
< rows = ["\t".join(str(r[k]) for k in ("rule", "findings", "cause_runs", "sole_cause_runs", "advice_runs")) for r in s["rules"]]
< print("rule rows in the table:", all(r in table for r in rows), len(rows))
< line = "repeat denials of a file already denied in the same session: %d, with the same causes: %d" % (s["repeat_denials"], s["repeat_denials_same_causes"])
< print("runs:", s["runs"], "repeat line in table:", line in table)
rule rows in the table: True 4
runs: 8 repeat line in table: True
```

## owner-reads-the-hook-record#stats-define-denial-advice-repeats-and-top/session-is-denied-three-times-on-one-file

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$ sh -c 'RUNE_STATE_DIR=state rune hook record stats | grep "^repeat denials"'
repeat denials of a file already denied in the same session: 2, with the same causes: 1
```

## owner-reads-the-hook-record#stats-define-denial-advice-repeats-and-top/top-cuts-a-tie

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$# From the 14th on, MD036, MD013, and Core.NoEmDash each caused one denial: the name decides.
$ sh -c 'RUNE_STATE_DIR=state rune hook record stats --top 1 --since 2026-10-14 | grep -A1 "^rule"'
rule	findings	cause	sole cause	advice
Core.NoEmDash	1	1	0	0
```

## owner-reads-the-hook-record#import-backfills-from-raw-claude-sessions/import-reads-two-transcripts-twice

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-store" store'
$ RUNE_STATE_DIR=state rune hook record import --harness claude store
read 2 transcript(s) under store: 4 record(s) added, 0 already held, into state/hooks/record
$# The checkpoint skips the bytes already read, so nothing is even offered again.
$ RUNE_STATE_DIR=state rune hook record import --harness claude store
read 2 transcript(s) under store: 0 record(s) added, 0 already held, into state/hooks/record
$ sh -c 'grep -c "refernce" state/hooks/record/*.jsonl | sed "s/.*://" | sort -u; grep -c "quick brown fox" state/hooks/record/*.jsonl | sed "s/.*://" | sort -u'
0
0
```

## owner-reads-the-hook-record#import-backfills-from-raw-claude-sessions/live-record-already-holds-the-tool-use

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-store" store'
$ sh -c 'mkdir -p state/hooks/record && printf "%s\n" "{\"v\":1,\"ts\":\"2026-10-02T10:00:01.000Z\",\"harness\":\"claude\",\"event\":\"tool.after\",\"native_event\":\"PostToolUse\",\"hook\":\"lint-on-write\",\"session_id\":\"s1\",\"tool_use_id\":\"toolu_3\",\"cwd\":\"/w\",\"file\":\"/w/b.md\",\"duration_ms\":50,\"exit_status\":0,\"asked\":\"deny\",\"answer\":\"deny\",\"failure\":null,\"metrics\":null}" > state/hooks/record/2026-10.jsonl'
$ RUNE_STATE_DIR=state rune hook record import --harness claude store
read 2 transcript(s) under store: 3 record(s) added, 1 already held, into state/hooks/record
$ sh -c 'RUNE_STATE_DIR=state rune hook record --json | grep -c toolu_3'
1
```

## owner-reads-the-hook-record#import-keeps-a-checkpoint/transcript-grows

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-store" store'
$ sh -c 'RUNE_STATE_DIR=state rune hook record import --harness claude store >/dev/null'
$ sh -c 'printf "%s\n" "{\"type\":\"assistant\",\"timestamp\":\"2026-10-03T10:00:00.000Z\",\"sessionId\":\"s1\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_5\",\"name\":\"Write\",\"input\":{\"file_path\":\"/w/late.md\"}}]}}" "{\"type\":\"attachment\",\"timestamp\":\"2026-10-03T10:00:30.000Z\",\"sessionId\":\"s1\",\"cwd\":\"/w\",\"attachment\":{\"type\":\"hook_blocking_error\",\"hookEvent\":\"PostToolUse\",\"hookName\":\"PostToolUse:Write\",\"toolUseID\":\"toolu_5\",\"blockingError\":{\"command\":\"sh /Users/x/.sd/hook PostToolUse\",\"blockingError\":\"late.md:1:1: [MD041] First line\"}}}" >> store/proj-a/one.jsonl'
$ RUNE_STATE_DIR=state rune hook record import --harness claude store
read 2 transcript(s) under store: 1 record(s) added, 0 already held, into state/hooks/record
```

## owner-reads-the-hook-record#import-keeps-a-checkpoint/attachment-refers-to-an-earlier-tool-use

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-store" store'
$ sh -c 'printf "%s\n" "{\"type\":\"assistant\",\"timestamp\":\"2026-10-03T10:00:00.000Z\",\"sessionId\":\"s1\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_5\",\"name\":\"Write\",\"input\":{\"file_path\":\"/w/late.md\"}}]}}" >> store/proj-a/one.jsonl'
$ sh -c 'RUNE_STATE_DIR=state rune hook record import --harness claude store >/dev/null'
$ sh -c 'printf "%s\n" "{\"type\":\"attachment\",\"timestamp\":\"2026-10-03T10:00:30.000Z\",\"sessionId\":\"s1\",\"cwd\":\"/w\",\"attachment\":{\"type\":\"hook_blocking_error\",\"hookEvent\":\"PostToolUse\",\"hookName\":\"PostToolUse:Write\",\"toolUseID\":\"toolu_5\",\"blockingError\":{\"command\":\"sh /Users/x/.sd/hook PostToolUse\",\"blockingError\":\"late.md:1:1: [MD041] First line\"}}}" >> store/proj-a/one.jsonl'
$ sh -c 'RUNE_STATE_DIR=state rune hook record import --harness claude store >/dev/null'
$ RUNE_STATE_DIR=state rune hook record --file late.md
2026-10-03T10:00:30.000Z claude  PostToolUse  lint-on-write  deny  deny      - /w/late.md  [MD041]
```

## owner-reads-the-hook-record#import-keeps-a-checkpoint/transcript-ends-mid-line

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-store" store'
$ sh -c 'printf "%s" "{\"type\":\"attachment\",\"timestamp\":\"2026-10-03T10:00:30.000Z\",\"sessionId\":\"s1\",\"cwd\":\"/w\",\"attachment\":{\"type\":\"hook_blocking_error\",\"hookEvent\":\"PostToolUse\",\"hookName\":\"PostToolUse:Write\",\"toolUseID\":\"toolu_6\",\"blockingError\":{\"command\":\"sh /Users/x/.sd/hook PostToolUse\",\"block" >> store/proj-a/one.jsonl'
$ RUNE_STATE_DIR=state rune hook record import --harness claude store
read 2 transcript(s) under store: 4 record(s) added, 0 already held, into state/hooks/record
$ sh -c 'printf "%s\n" "ingError\":\"x.md:1:1: [MD041] First line\"}}}" >> store/proj-a/one.jsonl'
$ RUNE_STATE_DIR=state rune hook record import --harness claude store
read 2 transcript(s) under store: 1 record(s) added, 0 already held, into state/hooks/record
```

## owner-reads-the-hook-record#the-record-stays-on-the-machine/release-runs-with-a-populated-record

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/support/deck" deck && RUNE_STATE_DIR=state rune release science --source deck >/dev/null 2>&1'
$ sh -c 'for t in deck/runes/science/dist/*.gz; do tar -tzf "$t" | grep -c "hooks/record" || true; tar -xzOf "$t" | grep -c "toolu_a1" || true; done | sort -u'
0
```

## owner-reads-the-hook-record#the-record-stays-on-the-machine/proof-runs-with-a-populated-record

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/filled-good" repo && RUNE_STATE_DIR=state rune proof run sample-change-one --pause 0 --source repo >/dev/null'
$ sh -c 'grep -rl "toolu_a1" repo || echo "no record line in the proof"'
no record line in the proof
```

## owner-reads-the-hook-record#the-record-stays-on-the-machine/proof-scene-reads-the-record

```console
$ sh -c 'mkdir -p state/hooks/record && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/hooks/record-lines/2026-10.jsonl" state/hooks/record/'
$# with the state dir named, the record lists its lines
$ sh -c 'RUNE_STATE_DIR=state rune hook record --json | wc -l | tr -d " "'
8
$# a proof scene runs the same command without naming a state dir, and its fence expects no output
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/proof/record-reader" repo && RUNE_STATE_DIR=state rune proof run sample-change-one --pause 0 --source repo | tail -n 1'
1 scenes, 1 proven, 0 unproven; transcript [..]
$ sh -c 'grep -rl "toolu_a1" repo || echo "no record line in the proof"'
no record line in the proof
```
