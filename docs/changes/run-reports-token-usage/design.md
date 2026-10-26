# Run reports token usage design

## Approach

The run reads the counts and the models from the structured output each harness already prints, instead of a second channel or an estimate. Claude runs with `--output-format json` and the run reads its one result object. Codex and opencode print JSON lines, and the run reads the events it knows. A count a harness does not print stays null: the run writes no zero it did not read.

## Structure

- Claude: the result object is read straight into its type, so a field the run does not report is skipped as it is read. The `--verbose` message array is judged by its last element. The output limit judges the decoded answer, and the stdout capture is seven output limits to hold the escaping.
- Codex: the last `turn.completed` carries the usage. The `error`, `turn.failed`, and error item events are the harness errors, each named for its event when it carries no message.
- OpenCode: the `step_finish` parts are summed field by field. A `session.error` fails the run with its message, or a stand-in when it names none.
- Harness errors: every text is cut to 4,000 characters with `…[cut]` at the cut, at the reader that collects it and again where a failure takes the list. Only the window around a stream's tail passes through redaction.

## Limits

- A non-JSON banner line on stdout that starts with `[` or `{` fails a good run. The run cannot tell such a line from a JSON line cut short, and a cut line may be the one that names a failure, so every line that starts a JSON value must read. A banner that starts with any other character is skipped.
- A codex failure event beside a final message file still passes the run with that file's text. The run fails on the event only when the file is absent.

## Risks

- A long answer in a failed result can fill the capture. The text is cut as it is read, the result is dropped before the tails are cut, and the ignored memory test runs a 54 MiB failed result against the bound.
- A harness may change its event shape. The readers fail a clean run on a line they cannot read, named by its line, instead of passing without the count.
