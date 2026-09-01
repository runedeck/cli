---
title: "Terminal Theme System"
description: "Named themes with light and dark pairing and token overrides feed one resolved palette for Sheet and TUI output"
type: adr
category: cli
tags:
    - cli
    - ux
    - theme
status: proposed
created: 2026-08-29
updated: 2026-08-29
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0007 Interactive Mode and TUI"
    - "CLI-0028 Setup Plan and Apply"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5"]
informed: []
upstream: []
---

# Terminal Theme System

## Context and Problem Statement

`src/cli/style.rs` carries one hardcoded truecolor palette, tuned for dark terminals.
On a light terminal the output is hard to read. The TUI keeps its own palette beside it.
[Herdr][HERDR] ships named built-in themes, follows the host terminal appearance, and lets a user
override single color tokens. Rune needs readable output on both appearances without a fork of
every render path.

## Decision Drivers

- Readable output on light and dark terminals
- One palette source for `Sheet` output and the TUI
- `NO_COLOR`, `--no-color`, and piped output keep their meaning
- Palette data must carry clean licensing

## Considered Options

1. **Keep the single palette** — no work, light terminals stay unreadable.
2. **Named theme set with light and dark pairing and token overrides** — a `[theme]` config
   selects a built-in palette, an automatic mode follows the host appearance where the terminal
   reports it, and single tokens stay overridable.
3. **User-defined theme files only** — maximum freedom, but every user starts from nothing and
   golden tests lose a stable reference.

## Decision Outcome

Option 2. The user config gains `theme.name`, `theme.auto_switch`, `theme.dark_name`,
`theme.light_name`, and a `theme.custom` token table. Built-in palettes come from their canonical
upstream projects (Catppuccin, Tokyo Night, Nord, and peers), with attribution recorded beside the
data. Theme resolution runs once at dispatch and produces one palette that `Sheet` and the TUI
consume. Appearance detection is best effort: when the terminal does not report its background,
the configured default applies. An unknown theme name warns and keeps the default, it never
aborts. `--no-color`, `NO_COLOR`, and non-terminal output suppress color before any theme applies.

## Consequences

- [+] Every command's output becomes readable on light terminals
- [+] One palette source removes the Sheet and TUI drift
- [+] Golden tests pin one theme and stay stable
- [-] Palette data and attribution need maintenance
- [-] Appearance detection stays unreliable in some terminals, so the config default matters

[HERDR]: https://github.com/herdrdev/herdr
