## ADDED Requirements

### Requirement: The runner reports each token count the harness prints

The runner MUST put a `usage` object in the JSON result with `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`, `reasoning_output_tokens`, and `total_tokens`. A count the harness prints MUST appear as that number. A count the harness does not print MUST stay null. The runner MUST NOT write zero for a count it did not read.

#### Scenario: Grok prints no count

- **WHEN** a grok run answers with `{"text":"ok"}`
- **THEN** every key of `usage` is null
- **AND** `completion_tokens` is null

#### Scenario: Agy prints only its output count

- **WHEN** an agy run answers with `usage` of `output_tokens` 17
- **THEN** `usage.output_tokens` is 17
- **AND** `usage.input_tokens` and `usage.total_tokens` are null

### Requirement: The runner reads claude's counts and adds its total

The runner MUST read claude's `--output-format json` result object and copy its `usage` counts. The total MUST be `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, and `output_tokens` added, because claude's input count excludes the cached tokens. The total MUST stay null when one of the four is missing or the sum overflows.

#### Scenario: Claude result carries four counts

- **WHEN** a claude run ends with `usage` of 11 input, 3 cache creation, 5 cache read, and 7 output tokens
- **THEN** `usage.total_tokens` is 26
- **AND** `completion_tokens` is 7

#### Scenario: Claude result omits a cache count

- **WHEN** a claude run ends with `usage` of 11 input and 7 output tokens and no cache counts
- **THEN** `usage.total_tokens` is null
- **AND** `usage.output_tokens` is 7

### Requirement: The runner reads codex's turn counts

The runner MUST copy the `usage` of the last `turn.completed` event codex prints. `cached_input_tokens` MUST fill `cache_read_input_tokens` and `cache_write_input_tokens` MUST fill `cache_creation_input_tokens`. The total MUST be `input_tokens` and `output_tokens` added, because codex's input count includes the cached tokens.

#### Scenario: Codex turn completes with counts

- **WHEN** a codex run prints `turn.completed` with 100 input, 60 cached, 10 cache write, 20 output, and 5 reasoning tokens
- **THEN** `usage` is 100 input, 10 cache creation, 60 cache read, 20 output, 5 reasoning, and 120 total
- **AND** the result has no `models` key

### Requirement: The runner sums opencode's step counts

The runner MUST add the `tokens` of every `step_finish` event opencode prints, field by field, and keep each reported `total` added as the run's total. A run whose steps print no counts, or only zero output tokens, MUST report every count null. A field whose sum overflows MUST stay null: the runner MUST NOT report a wrapped number.

#### Scenario: Two opencode steps report counts

- **WHEN** an opencode run prints two `step_finish` events with 5 and 10 input, 4 and 8 output, and 10 and 20 total tokens
- **THEN** `usage` is 15 input, 12 output, and 30 total
- **AND** the result has no `models` key

#### Scenario: Opencode step sum overflows

- **WHEN** an opencode run prints three `step_finish` events whose input counts are the largest 64-bit number, 1, and 7
- **THEN** `usage.input_tokens` is null
- **AND** the other sums are reported

### Requirement: The runner reads a malformed count as unknown

A count that is no non-negative integer in a codex or opencode event, or a number out of range, MUST read as unknown. The runner MUST still read the rest of the event, so an error event with a malformed count still fails the run with its message.

#### Scenario: Codex failed turn carries a negative count

- **WHEN** a codex run prints `turn.failed` with the message `turn aborted` and a `usage` whose `input_tokens` is -1, and no final message
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.harness_errors` is `["turn aborted"]`

#### Scenario: Opencode session error carries a negative count

- **WHEN** an opencode run prints `session.error` with the message `route failed` beside a `tokens` whose `input` is -1
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.message` is `opencode session error: route failed`

#### Scenario: Codex failed turn carries a count out of range

- **WHEN** a codex run prints a partial agent message, then `turn.failed` with the message `turn aborted` and a `usage` whose `input_tokens` is `1e999`, and no final message
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.harness_errors` is `["turn aborted"]`

#### Scenario: Opencode session error carries a count out of range

- **WHEN** an opencode run prints a partial text, then `session.error` with the message `route failed` beside a `tokens` whose `input` is `1e999`
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.message` is `opencode session error: route failed`

### Requirement: The runner fails a codex run on a failure event without a message

A codex `error` event, a `turn.failed` event, or an `item.completed` event whose item is an error MUST fail the run whatever its message holds. When the message is null, missing, or blank, the harness error MUST name the event instead, as `turn.failed names no message`, so a partial answer never passes as the result.

#### Scenario: Codex failed turn names no message

- **WHEN** a codex run exits 0 after a partial agent message and `turn.failed` whose `error.message` is null, with no final message file
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.harness_errors` is `["turn.failed names no message"]`

### Requirement: The runner cuts each harness error at a stated size

Each text in `details.harness_errors`, and the message of an opencode session error, MUST be cut to 4,000 characters, the size of a stream tail. A cut text MUST end with `…[cut]` inside that size, so a text already cut stays the same. The runner MUST cut as it reads, so it holds no copy of the full text, and the memory of a failed claude run MUST stay under the same multiple of the stdout capture as a run that answered.

#### Scenario: Failed claude result carries a long text

- **WHEN** a claude run exits 0 with subtype `error_max_turns`, `is_error` true, and a text of 10,000 characters
- **THEN** the run exits 1 with kind `surface_error`
- **AND** the one `details.harness_errors` text is 4,000 characters and ends with `…[cut]`

#### Scenario: Failed claude result fills the capture

- **WHEN** a claude run exits 0 with `is_error` true and a text of 54 MiB
- **THEN** the run exits 1 with kind `surface_error`
- **AND** the peak memory of the run stays under four times the stdout capture

### Requirement: The runner fails a clean run on a JSON line it cannot read

A line on stdout that starts a JSON object or array and that the runner cannot read MUST fail a run that exited clean. The message MUST name the tool, the line on stdout, and the reason. The runner MUST still read the lines around it, so a run that already failed keeps the errors they name. A line that starts no JSON value, such as a banner or an `API Error:` note, is no event and never fails the run.

#### Scenario: Codex clean exit prints a truncated event line

- **WHEN** a codex run exits 0 after an `item.completed` agent message and a `turn.failed` line cut before its closing braces
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.message` starts with `codex exec printed a JSON line rune cannot read (stdout line 2, column` and ends with `EOF while parsing an object)`

#### Scenario: Opencode clean exit prints a truncated event line

- **WHEN** an opencode run exits 0 after a `text` part and a `session.error` line cut before its closing braces
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.message` starts with `opencode run printed a JSON line rune cannot read (stdout line 2, column`

#### Scenario: Claude prints an unreadable line before its result

- **WHEN** a claude run prints an object line cut before its closing brace, then a success result
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.message` starts with `claude print printed a JSON line rune cannot read (stdout line 1, column`

### Requirement: The runner names the models claude reports

The runner MUST put the keys of claude's `modelUsage` map in `models`, sorted, and the map itself in `model_usage`. Both keys MUST be absent when the harness prints no `modelUsage` or an empty one. A model without a name MUST fail the run. `resolved_model` MUST stay the model the caller asked for. The model names, and every key and string value in `model_usage`, MUST pass the same credential redaction as `text` before the result is written anywhere.

#### Scenario: Claude names one model

- **WHEN** a claude run ends with `modelUsage` keyed by `claude-opus-4-6`
- **THEN** `models` is `["claude-opus-4-6"]`
- **AND** `model_usage.claude-opus-4-6` is that model's map

#### Scenario: Claude names no model

- **WHEN** a claude run ends with a result object without `modelUsage`
- **THEN** the result has no `models` key
- **AND** the result has no `model_usage` key

#### Scenario: Claude prints an empty model map

- **WHEN** a claude run ends with `modelUsage` of `{}`
- **THEN** the result has no `models` key
- **AND** the result has no `model_usage` key

#### Scenario: Claude echoes a credential into the model map

- **WHEN** a claude run on a profile with a route token ends with `modelUsage` keyed by that token's value, with the value inside a string of the map
- **THEN** `models` is `["<redacted>"]`
- **AND** neither stdout nor the `--output` file holds the value

### Requirement: The runner reads claude's result from a single object or a message array

The runner MUST read claude's result from the one JSON object `--output-format json` prints, or from the last element of the JSON array claude prints when `--verbose` or a `viewMode` of `verbose` is in effect. The runner MUST judge the array by that last element alone.

#### Scenario: Claude prints a message array

- **WHEN** a claude run prints an array of a `system` object, an `assistant` object, and a `result` object with subtype `success` and text `PING`
- **THEN** the run exits 0
- **AND** `text` is `PING`

### Requirement: The runner fails on a claude result that is no success

The runner MUST take the answer text from the `result` field of claude's result object. An `is_error` result, a `subtype` other than `success`, or a nonempty `errors` list MUST fail the run with exit 1 and kind `surface_error`, even when claude exits 0. One predicate MUST decide that verdict and which messages the failure carries. Output without a JSON result object MUST fail the run the same way.

#### Scenario: Claude reports an error and exits 0

- **WHEN** a claude run exits 0 with subtype `error_during_execution` and `errors` of `Provider quota exhausted`
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.harness_errors` is `["Provider quota exhausted"]`

#### Scenario: Claude lists errors beside a success

- **WHEN** a claude run exits 0 with subtype `success`, `is_error` false, text `done`, and `errors` of `tool denied`
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.harness_errors` is `["tool denied", "done"]`

#### Scenario: Claude fails a subtype without the error flag

- **WHEN** a claude run exits 0 with subtype `error_max_turns`, `is_error` false, and text `Reached the turn limit`
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.harness_errors` is `["Reached the turn limit"]`

#### Scenario: Claude prints plain text

- **WHEN** a claude run exits 0 with `plain text` on stdout
- **THEN** the run exits 1 with kind `surface_error`
- **AND** `details.stdout_tail` is `plain text`

### Requirement: The runner lists each claude error once

Claude's `errors` list, its `API Error:` lines, and the text of a failed result MUST be `details.harness_errors`, each once, in that order, each cut at the stated size. A failed result that names no message MUST still fail, with an empty list.

#### Scenario: Claude repeats an error message

- **WHEN** a claude run prints `API Error: 429 rate limited` twice and a failed result whose `errors` repeat that line and `quota gone`
- **THEN** `details.harness_errors` is `["API Error: 429 rate limited", "quota gone"]`

### Requirement: The output limit judges claude's decoded answer

The runner MUST judge a claude answer against the output limit after JSON decoding, so the escaping of the answer costs nothing against it. The runner MUST keep claude's stdout capture bounded by a fixed multiple of the output limit. A decoded answer over the limit MUST fail the run with exit 125 and kind `output_limit`, naming the ordinary limit. The memory of a claude run MUST stay under a stated multiple of the stdout capture: the runner MUST skip a field of the result it does not report as it reads, and MUST hold no copy of it.

#### Scenario: Escaped answer passes the limit its decoded form fits

- **WHEN** a claude run answers with 4,300,000 backslashes, whose JSON form is over the output limit
- **THEN** the run exits 0
- **AND** `text` holds the 4,300,000 characters

#### Scenario: Decoded answer exceeds the limit

- **WHEN** a claude run answers with one byte over the output limit
- **THEN** the run exits 125 with kind `output_limit`
- **AND** `details.limit` is the output limit

#### Scenario: Result carries 24 MiB of padding rune does not report

- **WHEN** a claude result object holds a field of 24 MiB that rune does not report, beside the answer `reply`
- **THEN** the run exits 0
- **AND** `text` is `reply`
