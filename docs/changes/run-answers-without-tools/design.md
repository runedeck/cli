# Run answers without tools: design

## Approach

A third value of the access mode, `AnswerOnly`, beside read-only and workspace-write. Each harness's argument builder maps it: claude to `--tools ""`, opencode to a deny-all permission, the other three to a refusal at the head of their invocation. This beat a separate flag such as `--no-tools`, which every place that reads the mode would also have to read, and a profile argument, which the run owns and drops.

## Structure

- `AccessMode::AnswerOnly`, label `answer-only`, a clap value like the others.
- `refuse_answer_only(invocation)` at the head of `invoke_codex`, `invoke_grok` and `invoke_agy`. Their mode matches map answer-only to the read-only branch, which keeps each match total while the refusal makes that branch unreachable.
- `claude_args`: permission mode `plan`, tools `""`, no allowed-tools list. `invoke_opencode`: `OPENCODE_PERMISSION` set to `{"*":"deny"}`.
- Tests: the claude arguments, the three refusals, the dry-run label.

## Commands

| Role | Command |
| --- | --- |
| The runner starts a Claude Code seat without tools | `claude --print --permission-mode plan --tools ""` |
| The runner starts an OpenCode seat without tools | `OPENCODE_PERMISSION='{"*":"deny"}' opencode run --format json` |

## Risks

- A harness update that lets codex, agy or grok run without tools is a one-line change in the refusal and a test row.
- `--tools ""` removes the built-in tools. A plugin or MCP tool a seat's settings load is a different switch. A red seat always runs clean, and clean claude loads no settings, so it has none.
