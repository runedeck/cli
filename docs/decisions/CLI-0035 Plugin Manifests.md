---
title: "Plugin Manifests"
description: "Declared plugin manifests with a post-install event, layered on the git-style dispatch"
type: adr
category: cli
tags:
    - cli
    - plugins
    - extensibility
status: proposed
created: 2026-08-29
updated: 2026-08-29
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0015 Git-Style External Command Dispatch"
    - "CLI-0033 Deck Discovery"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5"]
informed: []
upstream: []
---

# Plugin Manifests

## Context and Problem Statement

CLI-0015 dispatches unknown subcommands to `rune-*` executables, but a plugin declares nothing:
no description, no listing, no way to react to rune events. [Herdr][HERDR] pairs local executable
plugins with a manifest that declares actions and event hooks.

## Decision Drivers

- A plugin must be listable with its purpose before anyone runs it
- Deploy-time automation needs an event, not a wrapper script
- A broken plugin must never break an install

## Considered Options

1. **Keep bare dispatch** — no declarations, no events.
2. **Manifest plus a post-install event** — a `plugin.yaml` beside each plugin declares name,
   description, executable, and subscribed events; rune fires `post-install`.
3. **Full plugin marketplace** — discovery, installation, and versioning of plugins; premature
   before the manifest layer exists.

## Decision Outcome

Option 2. Plugins live under `~/.config/rune/plugins/<name>/` with a `plugin.yaml` declaring
`name`, `description`, `exec`, and `events`. `rune plugin list` shows them. After a successful
install, rune runs every plugin subscribed to `post-install` with one JSON event on stdin
(source, target, providers, deployed count). The executable path resolves inside the plugin's
own directory. A plugin failure prints one warning and never changes the install result.
Marketplace listing arrives later through the CLI-0033 topic pattern.

## Consequences

- [+] Plugins become visible, declared, and event-driven
- [+] The dispatch surface of CLI-0015 stays unchanged
- [-] One event only in this change; a richer event set needs its own review
- [-] Manifested plugins run with the user's permissions; listing is not endorsement

[HERDR]: https://github.com/herdrdev/herdr
