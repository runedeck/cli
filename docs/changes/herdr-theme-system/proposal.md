---
adr: "docs/decisions/CLI-0031 Terminal Theme System.md"
status: proposed
---

# Herdr Theme System

## Why

Rune paints one hardcoded dark palette. Light-terminal users get unreadable output, and the TUI
keeps a second palette beside `Sheet`. Herdr shows the fix: named themes, a light and dark pair
that follows the host terminal, and single-token overrides. Governing decision: CLI-0031.

## What Changes

- The user config gains `theme.name`, `theme.auto_switch`, `theme.dark_name`,
  `theme.light_name`, and a `theme.custom` token table.
- Rune ships named built-in palettes from their canonical upstream projects.
- Theme resolution runs once at dispatch. `Sheet` and the TUI consume one resolved palette.
- An unknown theme name warns and keeps the default.
- `--no-color`, `NO_COLOR`, and non-terminal output suppress color before any theme applies.

## Capabilities

- theme (new)

## Impact

- `src/cli/style.rs`, the TUI palette, `src/ontology.rs`, and the config reference
- New palette data module with upstream attribution
- Golden TUI snapshots pin one theme
