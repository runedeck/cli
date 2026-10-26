# Queue reaches a running session: design

## Approach

Every message goes into one ledger, the postbox, and reaches its session by the best way its *[harness][HARNESS]* offers. Codex has its own queue, so `rune queue post` hands the message to `codex queue` and records it as `queued`. Every other harness reads the postbox: a hook handler delivers the message at the session's next turn, or the session runs `rune queue read` because the rune skill tells it to. The ledger is plaintext and append-only, as the owner asked, so a person reads it with `cat` and nothing is ever lost.

This beat refusing the harnesses without a queue, which left four of five without messages. It also beat a resumed turn per harness, which starts a second turn or a copy of a session that is already running.

## Harness support

Each tool's help was read on 2026-10-25. The OpenCode server page was read the same day.

| Harness | Version | Own queue for a running session | What it has instead |
| --- | --- | --- | --- |
| codex | 0.154.0 | Yes: `codex queue --thread <UUID or exact name> --message <text>`, with `--remote <ws://, wss://, unix://, unix://PATH>` and `--remote-auth-token-env <VAR>` | Prints `Queued message <id> for thread <id>.` |
| claude | 2.1.310 | No | `--resume <id>` starts a turn. With `--bg`, a running session gets a copy instead. |
| agy | 1.4.7 | No | `--conversation <id>` resumes a conversation for a new turn. |
| grok | 1.0.62 | No | `--resume <id or title>` starts a turn. `agent stdio --leader` speaks the Agent Client Protocol to a shared leader. |
| opencode | 1.18.29 | Not verified | `run --attach <url> --session <id>` waits for the turn. The server documents `POST /session/:id/prompt_async`, answered with 204, but not what it does to a busy session. |

## The postbox

`~/.rune/postbox.tsv` holds every message of every session on the machine. The first line is a header that starts with `#`. Each event is one line of seven tab-separated columns: timestamp (UTC, milliseconds), id, state, sender, harness, session, message. A tab, newline, carriage return, or backslash in a column is escaped as `\t`, `\n`, `\r`, `\\`. A state change repeats the sender, harness, and session and leaves the message empty, so every line names whom it concerns.

| State | Written by | Meaning |
| --- | --- | --- |
| `posted` | `post` | The message waits for its session. |
| `queued` | `post`, after `codex queue` succeeds | Codex's own queue holds it. Nothing in rune hands it out again. |
| `read` | `read`, the hook | Its session has it. |

A message's state is its last line. An id is the milliseconds since the epoch and 32 random bits, so ids sort by time. Every write holds the file's lock, and a read takes its messages and appends their `read` lines under one lock, so two readers never take the same message. A line that is not an event, such as a hand edit, is skipped, and `list` warns about it.

## Delivery

| Harness | How a `posted` message arrives |
| --- | --- |
| codex | Its own queue at post time. With `--postbox`, the hook at the end of a turn. |
| claude | The hook: context at session start and before a prompt, the reason to keep working at the end of a turn. |
| grok, agy, opencode | `rune queue read --session <id or name>`, as the rune skill tells the session. |

The hook handler is the built-in adapter `postbox`, declared as `exec: [rune-hook-postbox]`. It reads by the payload's session id. Context needs the `context` capability (Claude Code, Gemini, grok) and keeping a turn going needs `turn_continue` (Claude Code, Codex, grok, Antigravity), from the dispatcher's protocol table. On an event where the harness has neither, the handler reads nothing, so no message is marked read and lost. `session.start` is a passive event. A dispatcher from before the hooks round dropped a passive event's answer, so a message read there was marked read and never shown (found on 2026-10-25 in a scratch install). The hooks round, on `main` since 7cfd91ae, keeps a passive event's context, and the encoder hands it to Claude Code's `SessionStart`. So the documented handler subscribes `session.start`, and a resumed session gets its messages at once. `rune hook install` writes the Claude Code and Codex hook tables only, and the dispatcher ignores a Claude Code registration that grok fires, so grok reads through the skill today.

`rune queue read` without `--session` finds its own session in `CLAUDE_CODE_SESSION_ID`, which held the session's id in a live Claude Code desktop session on 2026-10-25, or in `CODEX_THREAD_ID`, which Codex sets to the thread id for every shell command it runs since [openai/codex#10096][CODEXTHREADID], merged 2026-02-03. The same variables name the sender of a post, as `harness:id`.

## Structure

- `src/cli/queue/ledger.rs`: the line format, `fold`, `append`, `take_posted`, `new_id`, `now`, beside `ledger_tests.rs`.
- `src/cli/queue/inbox.rs`: `read`, `list`, the session variables, the sender, and the message text that `read` and the hook share.
- `src/cli/queue/mod.rs`: `post`, `queue_transport`, the `codex queue` arguments and receipt, and the dry run. `tests.rs` holds the unit tests and the hook tests.
- `src/cli/hook/adapters.rs`: the `postbox` adapter, `postbox_at` for tests.
- The variable that `--remote-auth-token-env` names is hidden in the dry run and in a failure's stream tails because the flag names it, whatever its name. Its value comes from the route's environment first, else the caller's.
- For Codex, the route resolves as it does for `rune run`. The profile's environment applies, its arguments are dropped with a warning, no middleware pre-step runs, and a docker or tmux wrapper is refused. A postbox post resolves no profile.
- `run` lends `read_text`, `parse_duration`, `failure_json`, `failure_exit_code`, and `print_outcome`. `surface` lends `require_success`. Nothing in `rune run` changes behavior.
- `run_requested` in `src/cli/mod.rs` also matches `queue`, so a usage error of `rune queue` exits 125.

## Commands

| Role | Command |
| --- | --- |
| Queue a message for a Codex session | `codex queue --thread=<session> --message=<text> --remote=unix://` |
| Reach another Codex app server | `codex queue ... --remote=<addr> --remote-auth-token-env=<VAR>` |

The equals form keeps a message that starts with `-` from reading as a flag. Codex 0.154.0 parsed `--thread=probe-name --message=-starts-with-dash` without a usage error.

Running Codex sessions live on its shared local app-server daemon. Without `--remote`, `codex queue` starts an embedded app server. On 2026-10-25, with the daemon running, that refused the `--config` override that the owner's `codex-proxy` wrapper adds: `cannot queue through an embedded app server while a local app-server daemon is running; remove configuration overrides or use --remote`. Through `codex-proxy` with `--remote unix://`, the same message reached the `talks/uroboros` session, which answered it. So the post sends `--remote=unix://` unless the caller names another server.

## Results

| Outcome | Exit | JSON `kind` |
| --- | --- | --- |
| Posted to the postbox | 0 | `posted`, with `id`, `sender`, and `postbox` |
| Queued by Codex, receipt read | 0 | `queued`, with `id`, `message_id`, and `thread_id` |
| Queued by Codex, no receipt line | 0 | `queued`, with null Codex ids and a warning |
| Read, list, dry run | 0 | `read`, `list`, `dry_run` |
| Route that names no harness | 125 | `unsupported_harness` |
| Usage or configuration error, `read` with no session | 125 | `configuration_error` |
| Postbox cannot be read or written | 125 | `postbox_error` |
| Codex exits nonzero | Codex's status | `exit`, with both stream tails |
| Time limit, signal, binary missing or not runnable | 124, 128 plus signal, 127, 126 | as `rune run` reports them |

## Risks

- The ledger grows without limit, and every read parses it whole. A rotation that keeps every line needs a later change.
- A session without the hook or the skill never reads its messages. `rune queue list --state posted` shows them.
- Codex changes its receipt line. The post then reports `queued` with null Codex ids and a warning, and a test pins the current line.
- No Codex daemon runs. Codex then cannot connect to `unix://`, its error passes through, and nothing is recorded.

[HARNESS]: ../../specs/glossary/harness.md
[CODEXTHREADID]: https://github.com/openai/codex/pull/10096
