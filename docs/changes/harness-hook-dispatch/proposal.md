---
adr: docs/changes/harness-hook-dispatch/adr.md
status: proposed
decisions: ["Hooks route through one dispatcher declared in the rune config"]
---

# Harness hook dispatch

## Why

Six harnesses, six ways to wire a hook. On this machine a bash dispatcher (`sh ~/.sd/hook <Event>`) fans Claude's six events and Codex's one out to parts under `~/.sd/hooks/`; dcg, git-ai, rtk, the tmux status, turn sealing, and session capture each register themselves; Gemini, Grok, Antigravity, and OpenCode get ad hoc plugins or nothing. rune copies a hook file into a provider tree and rewrites one path in a Claude `hooks.json`, and no provider's hook table is written (issue 69, "no hook rendering yet"). The harness protocols differ in event names, payload key case, decision fields, exit-code meaning, and timeout units, so a handler written for one harness reaches no other. The forge `dispatch` binary proved the shape for Claude alone.

## What Changes

- `hooks:` in `~/.config/rune/config.yaml` declares handlers once: the canonical events they subscribe to, order, timeout, failure policy, per-harness overrides. Eight canonical events, plus harness-qualified native extensions.
- `rune hook run --harness <h> --native-event <e>` is the one command every harness registers. It normalizes the payload, runs the handlers in order under the event's mode and budget, and encodes one canonical result into the harness's own answer.
- `rune install` writes one dispatcher entry per subscribed event into the Claude and Codex hook tables, tracks them in the manifest, keeps every foreign entry, and reports a hand-modified owned entry as a conflict.
- Handlers are deck hook runes and rune adapters. The first handler is the author identity at `session.start`, written into the checkout's jj and git config so every tool shell of the session commits as the model. dcg, git-ai, rtk, lint-on-write, session capture, the turn checkpoint, the tmux status, and the three jj guards (worktree, agent isolation, VCS internals) are adapters over the existing executables and parts. Install adopts the entries those tools wrote for themselves.
- **BREAKING** The `sd hook` dispatcher and its parts retire when this change lands: the dotfiles settings template stops declaring hooks and preserves the block rune writes.
- Two later changes: Gemini, Grok, Antigravity, and the OpenCode shim plugin; then `rune doctor` drift reporting.

## Capabilities

### New Capabilities

- `harness-hook-dispatch`: one config, canonical events, the dispatcher and its failure policy, ordered handlers with rewriters before guards, install ownership, adoption of predecessor and legacy entries.
- `checkout-author-identity`: the first handler, which writes the session's model identity into the checkout it works in.

### Modified Capabilities

None.

## Impact

- `src/cli/hook/` (new: config, events, dispatcher, encoders, adapters), `src/cli/install/` (hook tables and manifest), `src/cli/mod.rs` (`rune hook run`, `rune hook list`), `tests/hook_*.rs`, `docs/proofs/harness-hook-dispatch/`.
- Deck: hook runes gain frontmatter (event, handler, policy), written by the deck as its own capability under `define-module-architecture` with `harness-hook-dispatch#one-config-declares-every-hook` upstream.
- dotfiles: the Claude settings template becomes a modify script that keeps rune's `hooks` block; `dot_sd/hooks/` and the `sd hook` dispatcher go; the codex template keeps passing hooks through.
- Records: CLI-0020 (plugin deploy shape) is amended where it says hooks register through the plugin loader; ASSEMBLY-0005 no longer delegates event mapping to rulesync. Upstream in the deck: `define-module-architecture#one-validation-path`, `lint-enforced-foundations#checker-coverage`.
