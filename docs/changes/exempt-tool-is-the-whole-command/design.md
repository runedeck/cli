# Exempt tool is the whole command: design

## Approach

The guard is one more built-in adapter. The *[hook dispatcher][HOOKDISPATCHER]* spawns it as `rune hook adapter sandbox-guard` when a handler's `exec` names `rune-hook-sandbox-guard`. It reads the normalized payload, answers for Claude's `Bash` tool only, and passes every other harness and tool.

It judges `tool.input`, the effective input after every rewriter ordered before it. A rewriter such as rtk runs first and turns `jj log` into `rtk jj log`, and the harness runs the rewritten line. The [Claude Code hooks reference][CCHOOKS] says Claude Code evaluates permission rules against the input a hook returns, not the input Claude sent (read 2026-10-24). So `rtk jj log` runs outside the sandbox only when `rtk jj *` is exempt. When the rewritten line is refused, the reason quotes the line the model wrote from `raw.tool_input` beside the rewritten one, so the owner can see which pattern to add.

A small shell reader splits the line into simple commands. It finds each command's assignments, words, redirections, substitutions (`$( )`, `$(( ))`, backticks, `<( )`), the operator after it, and whether parentheses, braces, a loop, or a condition hold it. It expands nothing. Each word keeps its source text, so a suggestion repeats the model's own quoting.

A line runs outside the sandbox when every simple command matches an exempt pattern, has no assignment, is not grouped, has only stream joins (`2>&1`, `1>&2`) as redirections, and has no substitution. This is the rule the probes of 2026-10-22 measured. A substitution keeps the line inside even when it runs an exempt tool: `gh … "$(git config …)"` and `gh … $((0))` ran inside. In the suggested command, a word that held a substitution becomes `TEXT`, and the refusal says to write the text inline or to use the tool's file option. The guard refuses a sandbox-off request on such a line. It refuses a line that has an exempt tool and fails the rule. It passes every other line, with advice when separate commands are joined.

## Structure

- `sandbox_guard/shell.rs`: `parse(line) -> Vec<Command>`. A command has `assignments`, `words`, `redirects`, `substitutions`, `grouped`, and `joiner`. Here-document bodies and comments are skipped.
- `sandbox_guard/mod.rs`: `Exemptions` (patterns in the `excludedCommands` form), `exemptions_from(files)`, `judge(line, disable, exemptions)`, and the adapter `guard(payload, args)`.
- Wrapper words that are looked through to find the tool: `command`, `builtin`, `exec`, `nohup`, `time`, `env`, `sudo`, `nice`, `rtk`. Shells whose `-c` script is read: `sh`, `bash`, `zsh`, `dash`, `ksh`.
- `encode.rs`: Claude `PreToolUse` carries `additionalContext`. The hooks reference documents it as a "String added to Claude's context alongside the tool result" (read 2026-10-22). Before this change, the encoder reported that context as not expressible.

## Commands

| Role | Command |
| --- | --- |
| The dispatcher runs the guard | `rune hook adapter sandbox-guard [PATTERN…]` |
| A refusal names jj's directory option | `jj -R DIR` |
| A refusal names git's directory option | `git -C DIR` |
| A refusal names the runner's directory option | `rune run --repo DIR` |
| A refusal names codex's directory option | `codex exec -C DIR` |
| A refusal names opencode's directory option | `opencode run --dir DIR` |
| A refusal names gh's repository option | `gh -R OWNER/REPO` |

Each option was read from the tool's own help on 2026-10-22.

## Exempt list

Arguments, when given, replace the list and use the `excludedCommands` pattern form. Without arguments, the guard reads `sandbox.enabled` and `sandbox.excludedCommands` from the Claude Code settings files, lowest precedence first: `$CLAUDE_CONFIG_DIR/settings.json` (else `~/.claude/settings.json`), the project's `.claude/settings.json` and `.claude/settings.local.json` under `$CLAUDE_PROJECT_DIR` (else the payload's `cwd`), and the managed settings file. The patterns are joined. The last file that sets `enabled` decides. A sandbox that is off exempts nothing.

## Risks

- The reader is not a full shell grammar. A line it misreads can be refused wrongly. The refusal shows what it read, and the owner can drop the handler from the config.
- Claude Code may change its exemption rule. The probe is repeatable in one minute with `gh auth status`, which reads a path the sandbox denies.
- A refusal costs the model one turn. The owner accepted that cost for lines that are easier to read (2026-10-22).

[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
[CCHOOKS]: https://code.claude.com/docs/en/hooks
