---
adr: "docs/decisions/CLI-0036 TUI Status and First Run.md"
status: proposed
decisions: ["CLI-0036 TUI Status and First Run"]
---

# TUI first run

## Why

A terminal dashboard that opens on an empty list gives a new user no next step. [Designing Empty
States in Complex Applications][NNG] asks an empty area to say what it could show and to give a
direct path to the task that fills it. A dashboard that paints fixed dark colors ignores the theme
the user selected and a light terminal. A status bar that reports scan counts alone hides which
deck, target, and providers are active. Governing decision: CLI-0036.

## What Changes

- The TUI palette derives from the resolved theme. Light themes get light backgrounds and dark
  text.
- The status bar shows the deck name, the bound target, and one glyph per enabled provider.
- A first-run panel replaces the list when a completed scan finds no deck and no modules, and
  names the commands that lead out of it. A failed scan names its error in the footer instead.
- The help overlay names its close keys and the rune version, in theme colors.

## Capabilities

- dashboard-never-opens-empty (new)

## Impact

- `src/tui/styles.rs`, `src/tui/app.rs`, `src/cli/theme.rs`, and the TUI tests

[NNG]: https://www.nngroup.com/articles/empty-state-interface-design/
