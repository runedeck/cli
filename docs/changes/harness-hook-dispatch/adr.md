---
title: "Hooks route through one dispatcher declared in the rune config"
description: "Every harness registers rune hook run per event; handlers are declared once in the rune config against eight canonical events; the dispatcher normalizes the payload, runs handlers in order under a failure policy, and encodes one canonical result per harness; install owns only its entries"
type: adr
category: architecture
tags:
    - cli
    - hooks
    - harness
    - install
status: proposed
created: 2026-09-27
updated: 2026-09-27
author: "@N4M3Z"
project: cli
related:
    - "CLI-0020 Plugin Deploy Shape"
    - "CLI-0035 Plugin Manifests"
    - "ASSEMBLY-0005 Rulesync Interoperability"
    - "DOT-0007 Vendor Binaries on PATH and rune run for Automation"
    - "BRAIN-0011 Scoped Session Capture With Debt"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1", "gpt-6-astra"]
informed: []
upstream:
    - "define-module-architecture#one-validation-path"
    - "lint-enforced-foundations#checker-coverage"
change: harness-hook-dispatch
---

# Hooks route through one dispatcher declared in the rune config

## Context and Problem Statement

A hook is how a check runs on every write, how a guard refuses a command, how a session is captured, and how a commit gets its author. On this machine those hooks are wired six ways: a bash dispatcher for Claude and one Codex event, self-registering installers for dcg, git-ai, and rtk, JS plugins for OpenCode, cmux files for Grok, nothing for Antigravity. rune copies hook files and writes no table. The deck requires every check to run through one shared hook configuration and an enforcing check to be one linter invocation from it. The question is what owns the hook tables of six harnesses whose protocols agree on nothing but the idea of an event.

## Considered Options

1. Keep per-harness wiring and let rune render each harness's table from hook files as they are: no normalization, a handler per harness, the deck's one-configuration requirement unmet.
2. Extend the bash `sd hook` dispatcher to every harness: one dispatcher, but bash parts keyed on Claude tool names, no payload normalization, no per-harness encoder, and a config outside rune.
3. A rune dispatcher: handlers declared once in the rune config against canonical events, `rune hook run` per harness and event, normalized payload in, canonical result out, per-harness encoders, install owning its own entries.

## Decision Outcome

Option 3. The rune user config MUST declare each handler once under `hooks:` with its canonical events, order, timeout, and failure policy. The dispatcher MUST know eight canonical events and their native names per harness, MUST refuse a payload whose event disagrees with its argument, MUST run handlers serially by order under the event's mode (gate, collect, passive) and budget, and MUST encode one canonical result into the harness's documented answer, reporting an effect the harness cannot express instead of faking it. `rune install` MUST write one dispatcher entry per subscribed event, MUST inventory every foreign entry and keep it, MUST adopt the entries of the tools its adapters replace and the legacy dispatcher, and MUST stop on an owned entry changed by hand. A rewriter MUST be ordered before every guard. Release 1 covers Claude and Codex. The first handler is the author identity, resolved against `authors.yaml` and written into the checkout. The `sd hook` dispatcher retires when release 1 lands.

Option 1 leaves six wirings and no normalization. Option 2 would need a payload normalizer, a result encoder per harness, and a config schema written in bash and kept outside rune, beside the Rust adapters rune already has for launching every harness, and its parts key on Claude tool names today.

### Consequences

- [+] A handler is written once and reaches every harness that has the event. A missing event is a reported gap, not a silent skip.
- [+] The deck's one-configuration requirement is met by the rune config, and CI runs the same checker through `rune check`.
- [+] The author identity stops being a hand `jj metaedit` after every commit, because the checkout carries it.
- [-] Every hook event now pays rune's startup. The budget per event is explicit and clamped to each harness's cap: Claude's `SessionEnd` hooks share 1.5 s and Codex's `SessionEnd` allows 3 s at most, so session-end handlers only enqueue work.
- [-] The dotfiles settings template can no longer own the Claude hooks block. It becomes a modify script that keeps rune's block, and `rune install --check` reports drift between the two.
- [-] Grok reads Claude's settings, so a Claude registration also fires under Grok. The dispatcher recognizes Grok's environment and exits without a handler until the Grok adapter (change 2) registers Grok's own entries.
- [-] A Codex entry rune writes is inactive until the harness's trust step, which only the owner can take. Retiring the legacy Codex registration waits for that step.
- [-] OpenCode has no shell hooks. Until change 2 its plugins stay as they are.
