# Run names the session: design

## Approach

One optional string rides the invocation from the flag to each harness's argument builder. A builder whose harness has a flag for it appends the pair. The others leave it, and one table, `session_name_flag(surface)`, says which do. This beat an environment variable, which no harness reads for this, and a per-harness flag on the command line, which would make the caller know the harness a profile resolves to.

## Structure

- `SurfaceInvocation.session_name: Option<String>`, filled from `RunOptions.session_name`, filled from the clap flag.
- `session_name_flag(surface)`: claude `--name`, opencode `--title`, else `None`. The dry-run plan and the JSON result print it. A caller reads "not sent" from `session_name_flag: none` beside a set `session_name`.
- `claude_args` appends `--name <name>` after the model. `opencode_args`, split out of `invoke_opencode` so a test reads it, appends `--title <name>` before the profile's arguments.
- `owned_options` gains `-n` and `--name` for claude and `--title` for opencode.

## Commands

| Role | Command |
| --- | --- |
| The runner names a Claude Code session | `claude --name <name>` |
| The runner names an OpenCode session | `opencode run --title <name>` |

## Risks

- A harness adds a session-name flag later. The table gains a row and a test. Until then the plan says `none` for it.
- Under `--clean`, Claude Code runs with `CLAUDE_CONFIG_DIR` set to a scratch directory that the run removes at the end, so the named session is not found afterwards under the owner's config. The flag still names the session while it runs. The caller chooses `--clean`.
- A name is passed as one argument, whatever it holds. The harness decides what it accepts.
