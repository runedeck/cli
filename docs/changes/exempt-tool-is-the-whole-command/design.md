# Exempt tool is the whole command: design

## Approach

The guard is one more built-in adapter. The *[hook dispatcher][HOOKDISPATCHER]* spawns it as `rune hook adapter sandbox-guard` when a handler's `exec` names `rune-hook-sandbox-guard`. It reads the normalized payload, answers for Claude's `Bash` tool only, and passes every other harness and tool.

It judges `tool.input`, the effective input after every rewriter ordered before it. A rewriter such as rtk runs first and turns `jj log` into `rtk jj log`, and the harness runs the rewritten line. The [Claude Code hooks reference][CCHOOKS] says Claude Code evaluates permission rules against the input a hook returns, not the input Claude sent (read 2026-10-24). So `rtk jj log` runs outside the sandbox only when `rtk jj *` is exempt. When the rewritten line is refused, the reason quotes the line the model wrote from `raw.tool_input` beside the rewritten one, so the owner can see which pattern to add.

A small shell reader splits the line into simple commands. It finds each command's assignments, words, redirections, substitutions (`$( )`, `$(( ))`, backticks, `<( )`), the operator after it, and whether parentheses, braces, a loop, or a condition hold it. It expands nothing. Each word keeps its source text, so a suggestion repeats the model's own quoting.

A line runs outside the sandbox when every simple command matches an exempt pattern, has no assignment, is not grouped, has only stream joins (`2>&1`, `1>&2`) as redirections, and has no substitution. This is the rule the probes of 2026-10-22 measured. A substitution keeps the line inside even when it runs an exempt tool: `gh … "$(git config …)"` and `gh … $((0))` ran inside. In the suggested command, a word that held a substitution becomes `TEXT`, and the refusal says to write the text inline or to use the tool's file option. The guard refuses a sandbox-off request on such a line. It refuses a line that has an exempt tool and fails the rule. It passes every other line, with advice when separate commands are joined.

Cargo is the one exception to the chain advice (decided 2026-10-26). Cargo reads `.cargo/config.toml` from the current directory, never from the manifest's folder, and has no stable directory flag: `cargo -C DIR` fails on Rust 1.99 with "the `-C` flag is unstable". So `cd DIR && cargo …`, exactly two commands joined by `&&`, is not a chain, and the chain advice names cargo as the exception. A third command, a `;`, a pipe after cargo, or another program keeps the advice. The reverse case gets its own advice: a cargo call with `--manifest-path` whose first `.cargo/config.toml` (or `.cargo/config`) at or above the manifest's folder sets `build.target-dir` is told to run `cd <manifest folder> && cargo …` without the option, because from elsewhere cargo never sees that file and fills `<manifest folder>/target` with a private build. A relative manifest resolves against the payload's `cwd`. A file over 64 KiB, an unreadable or non-TOML file, a relative manifest with no `cwd`, and a call already under the checkout give no advice. The walk stops at the first config file, so a parent that sets the key while the nearer file does not leaves the guard silent, and `CARGO_TARGET_DIR` in the environment is not read. Both cost one private build and block nothing.

## Structure

- `sandbox_guard/shell.rs`: `parse(line) -> Vec<Command>`. A command has `assignments`, `words`, `redirects`, `substitutions`, `grouped`, and `joiner`. Here-document bodies and comments are skipped. A `$'…'` string ends at its first quote that no backslash escapes, so `gh api $'a\'b' > f` keeps its redirection. An assignment may quote its value (`FOO="1" jj log`) but not its name. A command, arithmetic, or backtick substitution inside `${…}`, such as `${X:-$(id)}`, counts like any other. The operand is scanned for `$(` and backticks without regard to quotes or `#`. Whether a quote in it quotes anything depends on the quotes around the expansion, so every substitution counts as one that runs, and `"${X:-'$(id)'}"` and `"${X:- # $(id)}"` are both caught. The end of a `${…}` inside double quotes is found with a single quote as a plain character, so `"${X:-'}"; rm -rf x` ends at the first `}` and holds a second command. The escapes of a `$'…'` string decode as in Bash: `\a`, `\b`, `\e`, `\f`, `\n`, `\r`, `\t`, `\v`, octal `\NNN`, `\xHH`, `\uHHHH`, `\UHHHHHHHH`, and `\cX`, so `$'\x6a\x6a' log > out` is `jj log` with a redirection. Bytes that form no UTF-8 become U+FFFD, a name no exempt pattern matches. A NUL ends the string as in Bash, so `$'jj\0suffix'` and `$'jj\x00suffix'` are `jj`, and the reader still reads on to the closing quote. Bash 5.3.20 printed `jj` for the hex, octal, Unicode, and NUL forms, and `'probe'` for `"${X:-'$(printf probe)'}"` (tried 2026-10-24). Unquoted, `${X:-'$(printf probe)'}` printed the text without running it, so there the scan refuses a line the shell would let through.
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
| The one chain that passes with no advice | `cd DIR && cargo …` |
| The advice for a manifest under a configured checkout | `cd <manifest folder> && cargo …` |

Each option was read from the tool's own help on 2026-10-22. Cargo's directory flag was tried on 2026-10-26.

## Exempt list

Arguments, when given, replace the list and use the `excludedCommands` pattern form. Without arguments, the guard reads `sandbox.enabled` and `sandbox.excludedCommands` from the Claude Code settings files, lowest precedence first: `$CLAUDE_CONFIG_DIR/settings.json` (else `~/.claude/settings.json`), the project's `.claude/settings.json` and `.claude/settings.local.json` under `$CLAUDE_PROJECT_DIR` (else the payload's `cwd`), and the managed settings file. The patterns are joined. The last file that sets `enabled` decides. A sandbox that is off exempts nothing.

## Risks

- The reader is not a full shell grammar. A line it misreads can be refused wrongly. The refusal shows what it read, and the owner can drop the handler from the config.
- Claude Code may change its exemption rule. The probe is repeatable in one minute with `gh auth status`, which reads a path the sandbox denies.
- A refusal costs the model one turn. The owner accepted that cost for lines that are easier to read (2026-10-22).

[HOOKDISPATCHER]: ../../specs/glossary/hook-dispatcher.md
[CCHOOKS]: https://code.claude.com/docs/en/hooks
