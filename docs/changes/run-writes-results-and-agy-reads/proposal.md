---
status: proposed
---

# Run writes results and agy reads

## Why

Three failures from 2026-10-21 and 2026-10-22 have one root: the runner depends on things the caller cannot control.

- The owner's Claude Code sandbox exempts a `rune` command only when the rune call is the whole command. A probe measured it: `rune run …` and `rune run … 2>&1` ran outside the sandbox, but `rune run … > file` and `rune run … | cat` ran inside it. There, codex cannot start its app server and agy cannot read its login. The runedeck/site session lost its codex and agy council seats to `> council/sol.md`. A caller that wants the result in a file has no way to get it without a redirect.
- A read-only agy seat ends with "agy print produced no final response" when the agent tries a shell command. Headless agy denies what it cannot ask, and the runner's error said nothing about the cause. An experiment then showed that agy writes files in plan mode and ignores the deny rules the runner can set, so agy cannot enforce read-only access at all.
- A clean grok run reports "Not signed in", because clean state drops the part of `~/.grok/config.toml` that maps each model to the environment variable holding its API key.

## What Changes

- `rune run --output <file>` writes the run's result to a file inside the repository, the temporary directory, or the rune state directory. Under `--json` stdout carries one short confirmation object, so no caller needs a redirect.
- A read-only agy run is refused, and the refusal points at workspace-write mode, which `run-isolates-write-runs` puts in its own jj workspace.
- When agy's log of the run shows a headless soft-deny, the failure names the denied tool in its hint.
- A clean grok run keeps only the `model.<id>.env_key` entries of the owner's grok configuration.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/mod.rs`, `src/cli/run/mod.rs`, a new `src/cli/run/output.rs`, `src/cli/surface/mod.rs`, their tests, `tests/run.rs`, `README.md`, `CHANGELOG.md`.
- The council mod and the HarnessCouncil skill can drop their output redirects.
- A caller that ran agy in read-only mode must switch to workspace-write mode.
