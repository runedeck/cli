# Run works inside a harness: design

## Approach

The runner stays a thin layer. It builds the child's environment in three steps, in this order: inherit the caller's environment, remove the parent harness's session variables, then apply the route's environment. A denylist of parent-session prefixes beat an allowlist of inherited keys, because an allowlist would drop the owner's own shell setup (`PATH`, proxy CA paths, tool homes) and break routes the owner configured outside rune.

The parent-session variables are the ones a harness sets for its own children, never ones the owner sets:

| Family | Removed |
| --- | --- |
| Claude Code and its desktop host, by exact name | `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_HOST_SESSION_ID`, `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_MESSAGING_SOCKET`, `CLAUDE_CODE_SESSION_ATTENDED`, `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`, `CLAUDE_CODE_SIMPLE`, `CLAUDE_CODE_DESKTOP_APP_VERSION`, `CLAUDE_CODE_EXECPATH`, `CLAUDE_CODE_TMPDIR`, `CLAUDE_CODE_ACCOUNT_UUID`, `CLAUDE_CODE_ORGANIZATION_UUID`, `CLAUDE_CODE_USER_EMAIL`, `CLAUDE_CODE_OAUTH_SCOPES`, `CLAUDE_CODE_TERMINAL_MCP_TOOLS`, `CLAUDE_CODE_STARTUP_FAILURE_RESULTS`, `CLAUDE_CODE_EMIT_TOOL_USE_SUMMARIES`, `CLAUDE_CODE_ENABLE_ASK_USER_QUESTION_TOOL`, `CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING`, `CLAUDE_CODE_EAGER_FLUSH`, `CLAUDE_CODE_DISABLE_CRON`, `CLAUDE_CODE_DISABLE_TERMINAL_TITLE`, `CLAUDE_CODE_TMUX_TRUECOLOR`, `CLAUDE_PID`, `CLAUDE_EFFORT`, `CLAUDE_TMPDIR` |
| Claude SDK and preview hosts, by prefix | every `CLAUDE_AGENT_SDK_*`, every `CLAUDE_PREVIEW_*` |
| Codex | `CODEX_SANDBOX`, `CODEX_SANDBOX_NETWORK_DISABLED`, `CODEX_THREAD_ID`, `CODEX_SESSION_ID` |
| Rune harness wrappers | `HARNESS_AUTOMATED` (already removed) |

The Claude names are the ones the desktop app set in a live session on 2026-10-20. They are exact, not a `CLAUDE_CODE_*` prefix, so an owner control with that prefix survives: `CLAUDE_CODE_CLIENT_CERT`, `CLAUDE_CODE_CLIENT_KEY`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_MAX_OUTPUT_TOKENS`, `CLAUDE_CODE_MAX_CONTEXT_TOKENS`, `CLAUDE_CODE_AUTO_COMPACT_WINDOW`. `CLAUDE_CONFIG_DIR` is not on the list: the owner may set it, and the clean run sets it on purpose.

An endpoint and its credentials form a family: Anthropic (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`) and OpenAI (`OPENAI_BASE_URL`, `OPENAI_API_KEY`). When the route sets any key of a family, every inherited key of that family the route does not set is removed from the child: an inherited credential never reaches a route endpoint, and an inherited endpoint never receives a route credential. An endpoint or credential a claude profile sets through `--settings {"env":{...}}` counts as route-set. When the route sets no key of a family, the inherited keys of that family pass together, so plain `claude` inside the desktop app keeps working through the app's gateway. Grok has no family: the owner's grok profile sets only `GROK_MODELS_BASE_URL` and authenticates with an owner-set key variable in the environment, which must stay inherited.

Under a tmux wrapper the removals travel inside the wrapped command line as `env -u KEY`; a docker container inherits nothing but the `-e` assignments, so it needs none.

## Structure

- One function, beside `process_request`, computes the removals from the invocation's environment. The launcher calls the same function, so `rune launch` gets the same isolation.
- `SurfaceFailure::Exit` and the timeout failure carry `stdout_tail` and `stderr_tail`: the last 40 lines or 4000 characters of each stream, whichever is shorter. The process layer keeps the partial output it read before a deadline.
- One hint table maps a sandbox signature in the tails (`Operation not permitted`, `-26276`, `failed to load CLI auth from keyring`) to a sentence. The text error and the JSON `hint` field print it.
- The dry run renders from the same argument builder the spawn uses, so the printed argv cannot drift from the started one. The prompt becomes `<prompt>`. Credential values become `<redacted>`, the placeholder the launch dry run already prints, and a credential-named key inside a `--settings` object or a `-c key=value` assignment is hidden too. The base URL comes from the profile key, else the route's environment, else the inherited value the isolation retains.
- `--json` routes the dry run, the check, and every configuration error through one printer.
- Grok clean state uses the existing `copy_optional_auth_file`.
- Claude workspace-write: `--permission-mode acceptEdits`, an explicit `--allowedTools` list of the readers and editors (`Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit`), and one `--settings` object that turns on Claude Code's sandbox with `failIfUnavailable` and `autoAllowBashIfSandboxed`. Bash is not on the allow list: an allow rule that matches a command also approves claude's unsandboxed retry of it, so the shell runs only inside the sandbox, and without `failIfUnavailable` a sandbox that cannot start falls back to an unsandboxed shell in silence. The run merges the profile's kept settings keys into that one object, because claude reads one `--settings` value; a profile's own `sandbox` keys are dropped with a warning that names the key, and the run's keys win.
- The codex and claude model filters compare the dropped value with the model the run passes and warn only when they differ. Every drop warning names the key and `<redacted>`, never the value, because a `mcp_servers.*.env` or `hooks` value may be a credential.
- Every failure and the harness's own stderr pass through one point that hides the route's credential values and the inherited ones the child keeps, before the tails are cut. Codex's error events parsed from stdout travel in `harness_errors`, apart from the raw stream tails, and every forced stop (deadline, forwarded signal, output limit) carries both tails.
- A sandbox signature counts on stderr, or in a harness-reported error for the certificate signatures; free model text on stdout never earns the hint, and a success never hints.

## Commands

| Role | Command |
| --- | --- |
| The runner starts a claude seat that may edit and build | `claude --print --permission-mode acceptEdits --allowedTools Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit --settings '{"sandbox":{"enabled":true,"failIfUnavailable":true,"autoAllowBashIfSandboxed":true}}'` |
| The runner shows its plan as JSON | `rune run <route> --dry-run --json` |

## Risks

- A harness may add a new session variable. A new Claude or codex name is a one-line table change with a test row; only the SDK and preview hosts match by prefix.
- Claude Code's sandbox cannot start inside another Seatbelt sandbox. With `failIfUnavailable` a nested run fails, and the failure carries the sandbox hint, instead of running a shell outside any sandbox. The owner runs the runner unsandboxed for implementation seats, as today.
- Removing an inherited Anthropic credential changes a route that relied on the leak. Such a route now fails with the endpoint's own 401, which the failure tail shows, and the profile must name its credential.
