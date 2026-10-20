# Run works inside a harness: design

## Approach

The runner stays a thin layer. It builds the child's environment in three steps, in this order: inherit the caller's environment, remove the parent harness's session variables, then apply the route's environment. A denylist of parent-session prefixes beat an allowlist of inherited keys, because an allowlist would drop the owner's own shell setup (`PATH`, proxy CA paths, tool homes) and break routes the owner configured outside rune.

The parent-session variables are the ones a harness sets for its own children, never ones the owner sets:

| Family | Removed |
| --- | --- |
| Claude Code and its desktop and SDK hosts | `CLAUDECODE`, every `CLAUDE_CODE_*`, every `CLAUDE_AGENT_SDK_*`, `CLAUDE_PID`, `CLAUDE_EFFORT`, `CLAUDE_TMPDIR`, every `CLAUDE_PREVIEW_*` |
| Codex | `CODEX_SANDBOX`, `CODEX_SANDBOX_NETWORK_DISABLED`, `CODEX_THREAD_ID` |
| Rune harness wrappers | `HARNESS_AUTOMATED` (already removed) |

`CLAUDE_CONFIG_DIR` is not on the list: the owner may set it, and the clean run sets it on purpose.

A credential is bound to its endpoint. When the route sets `ANTHROPIC_BASE_URL` and neither `ANTHROPIC_AUTH_TOKEN` nor `ANTHROPIC_API_KEY`, both are removed from the child. The same holds for `OPENAI_BASE_URL` and `OPENAI_API_KEY`. When the route sets no endpoint, an inherited endpoint and credential pass together, so plain `claude` inside the desktop app keeps working through the app's gateway.

## Structure

- One function, beside `process_request`, computes the removals from the invocation's environment. The launcher calls the same function, so `rune launch` gets the same isolation.
- `SurfaceFailure::Exit` and the timeout failure carry `stdout_tail` and `stderr_tail`: the last 40 lines or 4000 characters of each stream, whichever is shorter. The process layer keeps the partial output it read before a deadline.
- One hint table maps a sandbox signature in the tails (`Operation not permitted`, `-26276`, `failed to load CLI auth from keyring`) to a sentence. The text error and the JSON `hint` field print it.
- The dry run renders from the same argument builder the spawn uses, so the printed argv cannot drift from the started one. The prompt becomes `<prompt>`. Credential values become `<hidden>`. The base URL comes from the profile key or, failing that, the route's environment.
- `--json` routes the dry run, the check, and every configuration error through one printer.
- Grok clean state uses the existing `copy_optional_auth_file`.
- Claude workspace-write: `--permission-mode acceptEdits`, an explicit `--allowedTools` list (`Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit,Bash`), and one `--settings` object that turns on Claude Code's sandbox with `autoAllowBashIfSandboxed`. The run merges the profile's kept settings keys into that one object, because claude reads one `--settings` value.
- The codex and claude model filters compare the dropped value with the model the run passes and warn only when they differ.

## Commands

| Role | Command |
| --- | --- |
| The runner starts a claude seat that may edit and build | `claude --print --permission-mode acceptEdits --allowedTools Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit,Bash --settings '{"sandbox":{"enabled":true,"autoAllowBashIfSandboxed":true}}'` |
| The runner shows its plan as JSON | `rune run <route> --dry-run --json` |

## Risks

- A harness may add a new session variable. The denylist uses prefixes for the Claude family, and a new codex variable is a one-line table change with a test row.
- Claude Code's sandbox cannot start inside another Seatbelt sandbox. A nested run then has no shell, and the failure carries the sandbox hint. The owner runs the runner unsandboxed for implementation seats, as today.
- Removing an inherited Anthropic credential changes a route that relied on the leak. Such a route now fails with the endpoint's own 401, which the failure tail shows, and the profile must name its credential.
