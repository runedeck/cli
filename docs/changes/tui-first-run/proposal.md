---
adr: "docs/decisions/CLI-0036 TUI Status and First Run.md"
status: proposed
---

# TUI First Run

## Why

`rune tui` opens on a blank list when no deck is configured, paints one hard-coded dark palette
whatever `theme.name` says, and its status bar names nothing the user chose: no deck, no target,
no provider state. Herdr's TUI starts every session with the same three facts in view and leads a
new user to setup instead of an empty screen. Governing decision: CLI-0036.

## What Changes

- The TUI palette derives from the resolved theme. Light themes get light surfaces and dark text.
- The status bar shows the deck name, the bound target, and one glyph per enabled provider.
- A first-run panel replaces the empty list when the scan finds no deck and no modules, and names
  the commands that lead out of it.
- The help overlay names its close keys and the rune version, in theme colors.

## Capabilities

- tui (new)

## Impact

- `src/tui/styles.rs`, `src/tui/app.rs`, `src/cli/theme.rs`, and the TUI tests
- Closes tasks 1.5 and 2.3 of the herdr-theme-system change
