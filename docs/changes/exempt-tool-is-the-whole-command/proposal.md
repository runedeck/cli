---
adr: docs/changes/exempt-tool-is-the-whole-command/adr.md
status: proposed
decisions: ["The sandbox guard refuses an exempt tool the sandbox would keep, and advises on chains"]
---

# Exempt tool is the whole command

## Why

The owner's Claude Code sandbox lets `rune`, `jj`, `gh`, `git`, `codex`, `claude`, `agy`, `grok`, and `opencode` run outside it. It does so only when every command on the line is one of them, with nothing before it and no redirection to a file. A probe on 2026-10-22 showed it. `rune run …`, `rune run … 2>&1`, `jj a && jj b`, and `gh a | gh b` ran outside the sandbox. `FOO=1 rune run …`, `cd x && rune run …`, `bash -c 'rune run …'`, `command gh …`, `(gh …)`, `gh … > file`, `gh … 2>/dev/null`, `gh … | head`, `gh … $(printf x)`, `gh … "$(git config …)"`, and `agy … --print "$(cat file)"` ran inside it, where codex and agy fail. A request to turn the sandbox off also becomes a prompt the owner must answer, even for a command that already runs outside it.

The owner also finds a line of joined commands hard to read and to debug (2026-10-22).

## What Changes

- A new built-in *[hook][HOOK]*, the sandbox guard, runs on the Claude Code shell tool before each call.
- It refuses a sandbox-off request for a command that already runs outside the sandbox.
- It refuses an exempt tool that something on the line keeps inside the sandbox. The refusal names what keeps it inside and gives the command to run instead. A directory change before `jj`, `git`, `rune run`, `codex exec`, or `opencode run` becomes that tool's own directory option.
- A line that joins separate commands runs, and the model gets advice to split it into one command per call.
- A line with no exempt tool is never refused.
- The Claude encoder passes handler context on `PreToolUse` as `additionalContext`, which the hooks reference documents for that event.

## Capabilities

### New Capabilities

- `exempt-tool-is-the-whole-command`: what the sandbox guard refuses, what it advises, and where its exempt list comes from.

### Modified Capabilities

None.

## Impact

- New `src/cli/hook/sandbox_guard/` (the guard, its shell reader, tests). `src/cli/hook/adapters.rs` gains the name. `src/cli/hook/encode.rs` passes context on `PreToolUse`.
- New fixtures under `tests/fixtures/hooks/sandbox-guard*/`, and `docs/proofs/exempt-tool-is-the-whole-command/`.
- The owner's `hooks:` block gains one handler, `rune-hook-sandbox-guard`, on `tool.before`.
- The code sits on `harness-hook-dispatch` and `hooks-record-log`, and goes into release 0.6.0 with them.

[HOOK]: ../../specs/glossary/hook.md
