# Queue reaches a running session: design

## Approach

The route resolves as it does for `rune run`: `launch::resolve` gives the *[harness][HARNESS]*, the binary, the profile's arguments, and the environment. One table, `queue_transport(surface)`, says how a message reaches a running session of that harness, or why it cannot. Codex is the one row with a transport. The command builds the `codex queue` argument list, starts it under the process supervisor that `rune run` uses, and reads the receipt line from its stdout.

This beat a queue kept by rune itself, which every harness would have to read through a hook. It also beat a resumed turn per harness, which starts a second turn or a copy of a session that is already running.

## Harness support

Each tool's help was read on 2026-10-25. The OpenCode server page was read the same day.

| Harness | Version | Queue into a running session | What it has instead |
| --- | --- | --- | --- |
| codex | 0.154.0 | Yes: `codex queue --thread <UUID or exact name> --message <text>`, with `--remote <ws://, wss://, unix://, unix://PATH>` and `--remote-auth-token-env <VAR>` | Prints `Queued message <id> for thread <id>.` |
| claude | 2.1.310 | No | `--resume <id>` starts a turn. With `--bg`, a running session gets a copy instead. `attach` and `logs` read a background session. |
| agy | 1.4.7 | No | `--conversation <id>` resumes a conversation for a new turn. `remote-control` manages a daemon and has no send command. |
| grok | 1.0.62 | No | `--resume <id or title>` starts a turn. `agent stdio --leader` speaks the Agent Client Protocol to a shared leader, which is not a queue command. |
| opencode | 1.18.29 | Not verified | `run --attach <url> --session <id>` waits for the turn. The server documents `POST /session/:id/prompt_async`, answered with 204, but not what it does to a busy session. `serve` takes a random port by default, so the server address must come from the caller. |

## Structure

- `src/cli/queue/mod.rs`: `QueueOptions`, `execute`, `queue_transport`, `codex_queue_args`, `parse_receipt`, and the dry-run plan. `tests.rs` beside it holds the unit tests.
- The profile's arguments start a session, so they do not reach `codex queue`. One warning names them, with credentials hidden.
- No middleware pre-step runs: `codex queue` makes no model call. A docker or tmux wrapper is refused, as `rune run` refuses it.
- The child's environment is the caller's, minus the parent session's variables, plus the route's environment. This is the isolation `rune run` applies.
- `run` lends its message reader (`read_text`, which `read_prompt` now calls), `parse_duration`, `failure_json`, `failure_exit_code`, and `print_outcome`. `surface` lends `require_success`. Nothing in `rune run` changes behavior.
- `run_requested` in `src/cli/mod.rs` also matches `queue`, so a usage error of `rune queue` exits 125.

## Commands

| Role | Command |
| --- | --- |
| Queue a message for a Codex session | `codex queue --thread=<session> --message=<text> --remote=unix://` |
| Reach another Codex app server | `codex queue ... --remote=<addr> --remote-auth-token-env=<VAR>` |

The equals form keeps a message that starts with `-` from reading as a flag. Codex 0.154.0 parsed `--thread=probe-name --message=-starts-with-dash` without a usage error.

## The local daemon is the default server

Running sessions live on Codex's shared local app-server daemon. Without `--remote`, `codex queue` starts an embedded app server. On 2026-10-25, with the daemon running, that refused the `--config` override that the owner's `codex-proxy` wrapper adds: `cannot queue through an embedded app server while a local app-server daemon is running; remove configuration overrides or use --remote`. The same command through `codex-proxy` with `--remote unix://` passed that check and looked up the session, and so did plain `codex` without the override. So the queue sends `--remote=unix://` unless the caller names another server. A route's wrapper then works, and the binary stays the one the route names.

## Results

| Outcome | Exit | JSON `kind` |
| --- | --- | --- |
| Queued, receipt read | 0 | `queued`, with `message_id` and `thread_id` |
| Queued, no receipt line | 0 | `queued`, with null ids and a warning |
| Dry run | 0 | `dry_run` |
| Harness without a queue | 125 | `unsupported_harness` |
| Usage or configuration error | 125 | `configuration_error` |
| Codex exits nonzero | Codex's status | `exit`, with both stream tails |
| Time limit, signal, binary missing or not runnable | 124, 128 plus signal, 127, 126 | as `rune run` reports them |

## Risks

- Codex changes its receipt line. The command then reports `queued` with null ids and a warning, and a test pins the current line.
- No daemon runs. Codex then cannot connect to `unix://`, and its error passes through with its exit status.
- A busy app server makes `codex queue` wait. `--timeout` stops it, and no limit applies unless it is set, as for `rune run`.
- A harness gains a queue. The table gains a row and a test, and the refusal for it goes away.

[HARNESS]: ../../specs/glossary/harness.md
