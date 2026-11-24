---
name: rune
description: "Drive the rune CLI to stage, deploy, and verify runes (skills, agents, rules) from a deck into AI harness directories. USE WHEN install runes, add skills to a project, deploy deck content, check drift, .rune manifest, rune validate, spec-driven change, rune spec, message another session, read the postbox, rune queue. Not for authoring deck content (use the deck's build-* skills)."
version: ${VERSION}
---

# rune

rune deploys markdown instruction files (runes) from a deck into provider directories (`.claude`, `.codex`, `.gemini`, `.opencode`). The consumer manifest is `.rune` at the repo root; deployment records live in per-provider `.manifest` files. For Claude Code, skills, agents, and hooks deploy as the `rune` skills-directory plugin (`.claude/skills/rune/`), so skills invoke namespaced: `/rune:deslop`.

## Orientation

Run `rune context --json` first: it reports the acting root and role, the target binding, the manifest selection, which providers are deployed, active spec changes, and suggested next steps. `rune status --source <deck>` is the deck-side dashboard.

## Consumer Flow

```sh
rune add development                 # stage a whole deck domain
rune add --cast development          # stage a cast (named selection)
rune add development/skills/deslop   # stage one rune by qualified id
rune install                         # assemble and deploy to all providers
rune drift --target .                # deployed content matches source?
rune doctor --target .               # integrity: ok / modified / missing / orphan
```

`rune add` asks before staging into the bound target when the current directory has no `.rune`; non-interactive runs refuse the redirect, so run staging commands from the repo that carries `.rune`. Ambiguous ids fail loudly listing every candidate; retry with the qualified form `<domain>/<kind>/<name>`.

## Safe Scripting

- Inspection commands (`context`, `status`, `doctor`, `drift`, `provenance`, `config`, `spec …`) accept `--json` for structured output; staging commands (`add` and the kind adds) print plain text.
- `rune install --dry-run` previews pruning without writing.
- `rune doctor` is read-only. `rune repair` restores missing files, quarantines orphans, trashes orphan sidecars, and renames stale subjects; it never overwrites user edits. Overwriting edits takes a deliberate `rune install --force`.
- Never edit deployed files under provider directories; edit the deck source and reinstall. Local exceptions live in a rune's `user/` override.

## Spec Lifecycle

```sh
rune spec propose <change-id> --capability <name>
rune spec list                       # active changes; --specs for capabilities
rune spec show <id>                  # one change or capability spec
rune spec context <id>               # agent-ready work order
rune spec doctor                     # relationship health
rune spec archive <id>               # refuses on unchecked tasks; --abandon drops
```

## Messages Between Sessions

Sessions on this machine message each other through `rune queue`. A message waits in the postbox, `~/.rune/postbox.tsv`, until its session reads it. Codex sessions also get it through their own queue.

```sh
rune queue post codex --session <thread id> "Is the deck ready?"
rune queue post claude --session <session id> --message-file question.md
rune queue read                      # this session's messages; marks them read
rune queue read --session <name>     # where no session variable names you
rune queue list --state posted       # look without marking anything read
```

Claude Code and Codex sessions with the `postbox` hook get their messages at the next turn. Any other session runs `rune queue read --session <its id or name>` when a task starts and between steps. Answer a message by posting to its sender, who is named as `harness:session`. Name a Codex session by its UUID: Codex searches its server page by page for a name, which can take a minute or more.

## Turn Wrapping

Where the owner switched it on, the `turn-wrap` hook records each of your turns as one jj change. On Claude Code the owner sees a card with the change, the review command, and the undo. You do not see it, and Codex shows nothing on Stop. Read the result with `jj log` when you need it. The switch is `rune hook wrap on`, per jj workspace, and only the owner runs it. Never run it, `off`, or `reset` yourself, and never ask to. A workspace that more than one session touched after the last wrap is not wrapped, and a turn after a Stop hook failed or timed out waits for the next Stop. The owner sorts the working copy out and runs `rune hook wrap reset`. Keep working.

## Validation

`rune validate` checks schemas and structure. Add `--scan` only in commit and push hooks; it runs gitleaks and semgrep and is slow.
