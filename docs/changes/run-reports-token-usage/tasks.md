## 1. Implementation

- [x] 1.1 `TokenUsage` with six optional counts on `SurfaceReply`, plus `models` and `model_usage`
- [x] 1.2 Claude runs with `--output-format json`. The run reads the result object: text, `usage` with the summed total, `modelUsage`, error results, and plain-text output as a `surface_error`
- [x] 1.3 Codex copies the last `turn.completed` usage with the cache counts mapped. OpenCode sums every `step_finish`. Agy keeps `output_tokens`. Grok reports none
- [x] 1.4 The JSON result and the `--output` file carry `usage` with null unknowns, and `models` and `model_usage` only when reported
- [x] 1.5 CHANGELOG entry
- [x] 1.6 Review fixes. `models` and `model_usage` pass the credential redaction. A malformed codex or opencode count is unknown and never drops its event. Claude's `--verbose` array is read by its last element. A result with errors fails on one predicate that also picks the messages, each once. An empty `modelUsage` emits no key. An overflowing opencode sum is unknown. The output limit judges claude's decoded answer under a bounded capture
- [x] 1.7 Second review fixes. A count out of range (`1e999`) is unknown and never drops its event. A JSON line rune cannot read fails a clean codex, opencode, or claude run with a message that names the line. The memory of a claude run stays under a stated multiple of the stdout capture: the result is read straight into its type, a field rune does not report is skipped, the `--verbose` array keeps the text of one element, `modelUsage` is capped, a stop keeps only the window around its tail through redaction, and a redaction pass drops its copy before the next
- [x] 1.8 Third review fixes. A codex failure event without a message fails the run, named for the event. Each harness error text is cut to 4,000 characters with `…[cut]` at the cut, at the reader that collects it and where a failure takes the list. A failed claude result goes before its tails are cut, and only the window around a tail passes through redaction. The design records the banner-line limit

## 2. Verification

- [x] 2.1 Surface tests: claude result parsing, the error messages, malformed results, the total rules, the codex mapping, the opencode sum
- [x] 2.2 Run tests: claude usage and models in the result file, no models without `modelUsage`, an error result with exit 0, plain text, codex, opencode, grok
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo test --all-features`, `prek run --all-files`
- [x] 2.6 One surface test and one run test per review fix in 1.6. `cargo fmt --check`, clippy, and `cargo test --all-features` rerun
- [x] 2.7 Unit tests in the surface module and fake-harness run tests for 1.7: a count out of range in a codex and an opencode failure event, a truncated event line on a clean codex, opencode, and claude run, and 24 MiB of padding in a claude result. An ignored run test reads the peak memory of its children from `getrusage` over eight fake claude outputs, a stdout and stderr flood included, and asserts the four-times bound. Its peak is 169 MiB on macOS. `cargo fmt --check`, clippy, and `cargo test --all-features` rerun
- [x] 2.8 Unit tests in the surface module and fake-harness run tests for 1.8: codex failure events with a null, missing, or blank message, a claude failed result without a message, the cap on a long text in a claude result, a codex event, and a failure's list, a codex failed turn without a message after a partial answer, and a claude failed result of 10,000 characters. Each fails with the fix edited back by hand. The ignored memory test gains a failed claude result of 54 MiB and fails without the fix. `cargo fmt --check`, clippy, and `cargo test --all-features` rerun
- [ ] 2.4 `rune spec validate run-reports-token-usage` by hand: the session could not run it directly, only through the prek hooks
- [ ] 2.5 Live: `rune run sol@claude --json "Reply OK"` reports `usage.total_tokens` and `models`, and `rune run codex --json "Reply OK"` reports `usage.total_tokens`
