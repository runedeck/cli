---
status: proposed
decisions: ["The run reports the token counts and model names a harness prints, and nothing where it prints none"]
---

# Run reports token usage

## Why

A council mod runs one `rune run --json` per seat and wants to know what each seat cost and which model answered. Today the JSON result carries a `usage` object with every count null, and a count only from agy. Claude Code prints the counts and the models in its `--output-format json` result object. Codex prints them in its `turn.completed` event. OpenCode prints them in each `step_finish` event. Claude's and codex's formats were read from their published docs and opencode's from its event types on 2026-10-26. Grok's `{"text"}` object names no count and no model. Agy names only `output_tokens`. Both shapes come from today's code alone.

## What Changes

- The run reads claude's `--output-format json` result object instead of plain text: the text, the `usage` counts, the `modelUsage` map, and an error result that exits 0. The message array `--verbose` prints is read by its last element.
- The `usage` object gains `reasoning_output_tokens`. Claude's total is its four input and output counts added. Codex's total is its input and output added. OpenCode's total is the sum its steps report. A count a harness does not report stays null, and so does a sum that overflows. The run writes no zero for a count it did not read.
- The result gains `models`, the names claude's `modelUsage` lists, and `model_usage`, that map itself. Both are absent when the harness reports none or an empty map. Both pass the credential redaction the text passes.
- Claude's `errors` list and the text of a failed result join `details.harness_errors`, each once. A result with errors fails whatever its flags say. Output with no JSON result object, or a negative count in claude's result, fails the run with the kind `surface_error`. A negative count or a number out of range in a codex or opencode event is unknown. The event itself still counts.
- A JSON line rune cannot read never drops in silence. A clean codex, opencode, or claude run fails on it with a message that names the tool, the line on stdout, and the reason. A run that already failed keeps the errors the lines around it name.
- A codex failure event fails the run whatever its message holds. One with a null, missing, or blank message is named for the event, as `turn.failed names no message`, the way an opencode session error without a message is.
- Each harness error text is cut to 4,000 characters, the size of a tail, and ends with `…[cut]` when cut. The runner cuts as it reads, so a failed claude result whose text fills the capture costs no copy of it, and only the window around a tail passes through redaction.
- The output limit judges claude's answer after JSON decoding. The stdout capture grows to a fixed multiple of the limit to hold the escaping. The memory of a run stays under a stated multiple of that capture: the runner skips a field of the result it does not report as it reads, holds no copy of it, and carries only the window around a stop's tail through redaction.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/run/mod.rs`, `src/cli/surface/mod.rs`, `src/cli/surface/tests.rs`, `tests/run.rs`, `CHANGELOG.md`. The design records the limits of the readers.
- The claude argument list carries `--output-format json` where it carried `text`. The run owns that flag, so no profile sets it.
- Grok and agy still report no models. Grok reports no counts. The requested model stays `resolved_model`. The reported names are a separate list.
