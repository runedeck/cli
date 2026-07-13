---
title: "Entire Git-Hook Coexistence"
description: "Track Entire's pre-push wrapper and the rune pre-push validation check as pre-push.pre-entire to stop hook drift"
type: adr
category: cli
tags:
    - cli
    - hooks
    - entire
status: accepted
created: 2026-06-23
updated: 2026-06-23
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0008 Validation Script Distribution"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream:
    - "https://github.com/entireio/cli/issues/1250"
---

# Entire Git-Hook Coexistence

## Context and Problem Statement

rune sets `core.hooksPath = .githooks` and tracks its git hooks as source. Entire (a per-developer AI-session recorder) installs hooks into that same directory and wraps the tracked `pre-push`: it backs the rune pre-push validation check up to `pre-push.pre-entire`, then replaces `pre-push` with a marked shim that calls `entire hooks git pre-push` and chains to the backup. Because `pre-push` is tracked, that modification surfaces as working-tree drift on every machine running Entire and has been accidentally committed. A `.gitignore` entry cannot suppress a modification to an already-tracked file.

## Decision Drivers

- The working tree must stay clean for developers who run Entire — no recurring `pre-push` diff
- The rune validation pre-push validation check must keep running for everyone, including contributors without Entire
- No silent loss of the pre-push validation check: a hand-merged marker hook is overwritten by Entire with no backup
- rune's tracked-hook model must not be abandoned

## Considered Options

1. **Gitignore the Entire artifacts** — hides the per-machine capture hooks but cannot touch the tracked `pre-push` modification; the drift persists
2. **Remove Entire's hooks here** (`entire agent remove`) — restores the pure pre-push validation check but loses session capture and checkpoints
3. **Track all of Entire's hooks as source** — commit `pre-push` (Entire's wrapper), `pre-push.pre-entire` (the pre-push validation check), and the four capture hooks, the same way rune's own hooks are tracked

## Decision Outcome

Chosen: **track all of Entire's hooks as source.** Every file in `.githooks/` is committed, including the ones Entire installs:

- `pre-push` is Entire's wrapper: it calls `entire hooks git pre-push` when Entire is on `PATH`, then runs the rune pre-push validation check.
- `pre-push.pre-entire` is the rune pre-push validation check (the original `pre-push`, under the name Entire gives the file it displaces).
- `commit-msg`, `post-commit`, `post-rewrite`, and `prepare-commit-msg` are Entire's capture hooks.

Each is guarded by `command -v entire`, so on a machine without Entire they are no-ops and the wrapper still runs the pre-push validation check via the `else` branch. Only `.entire/` (runtime session data) and `.specstory/` (a separate tool) are gitignored.

This is the only arrangement that keeps both Entire and rune's tracked-hook model working. Entire wraps any hook that does not already carry its `Entire CLI hooks` marker; once the marked wrapper and its `pre-push.pre-entire` backup are committed, `entire enable` and `entire configure --force` find them already in place and rewrite nothing. A hook hand-edited to fold the pre-push validation check into the marked file would instead be overwritten by Entire with no backup — which is why the wrapper is committed exactly as Entire generates it.

Entire is standard rune kit, so the `rune init` template ships the same hook set: every scaffolded module inherits this coexistence from the start. The template's `pre-push` is the wrapper, `pre-push.pre-entire` is the pre-push validation check (carrying the `${VALIDATE_SH_SHA}` placeholder rune substitutes at init), and a new `templates/init/.gitignore` ignores the `.entire/` and `.specstory/` runtime data.

## Consequences

- [+] The working tree stays clean — an Entire reinstall changes nothing.
- [+] The pre-push validation check runs on every push path: jj (the `jj-push` alias), git with Entire (wrapper, then pre-push validation check), and git without Entire (the wrapper's `else` branch runs the pre-push validation check).
- [+] Every rune module is Entire-ready out of the box, with the pre-push validation check intact.
- [-] The tracked `pre-push` is Entire's wrapper; the pre-push validation check itself lives in `pre-push.pre-entire`.
- [-] If Entire changes its wrapper template, the committed `pre-push` needs a one-time re-commit per module (the pre-push validation check never changes). A future `rune` helper can regenerate and re-sync it across modules.

## More Information

- [CLI-0008 Validation Script Distribution](CLI-0008 Validation Script Distribution.md) — the pre-push validation check now hosted at `pre-push.pre-entire`
- Upstream `entireio/cli#1250` (external hooks backend) would let rune keep its pure tracked pre-push validation check once shipped
