---
title: "TUI Status and First Run"
description: "The TUI paints from the resolved theme, shows deck, target, and provider states, and routes an unconfigured root into setup"
type: adr
category: cli
tags:
    - cli
    - ux
    - tui
status: proposed
created: 2026-09-02
updated: 2026-09-02
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0007 Interactive Mode and TUI"
    - "CLI-0028 Setup Plan and Apply"
    - "CLI-0031 Terminal Theme System"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5"]
informed: []
upstream: []
---

# TUI Status and First Run

## Context and Problem Statement

CLI-0031 promised one palette for `Sheet` output and the TUI, but `src/tui/styles.rs` kept its
hard-coded dark constants and the app painted named terminal colors beside them. A user who
selects a light theme gets themed CLI output and an unchanged dark TUI. The status bar reports
scan counts and nothing the user configured. On a root without a deck the list reads `no rows`.
[Herdr][HERDR] opens every session with the workspace facts in view and leads a new user to
setup. Rune needs the same three things without a second configuration surface.

## Decision Drivers

- One palette source, as CLI-0031 requires
- The status bar shows what the user chose: deck, target, providers
- An unconfigured root leads into `rune setup`, never a blank list
- No new configuration keys

## Considered Options

1. **Surface tokens per theme** — add background and text tones to every palette. Six palettes
   to maintain, and custom overrides grow.
2. **A light flag per theme with derived surfaces** — the five tones keep the meaning, one bit
   picks the surface set, and the TUI derives the rest.
3. **Terminal default backgrounds** — paint no backgrounds and inherit the terminal. Selection
   and diff highlights lose contrast on unknown backgrounds.

## Decision Outcome

Option 2. `ThemeTones` gains `light`; `styles::Palette::from_theme` derives every TUI color from
the tones and that flag, and accessor functions replace the constants. The app detects provider
states once at load through the shared registry and reads the bound target once; the status bar
shows the deck name, the target, and one glyph per enabled provider. After a scan that finds no
deck and no modules, the TUI draws a first-run panel that names the root and the commands that
configure a deck, and the footer points at `rune setup`. The help overlay names its close keys
and the version.

## Consequences

- [+] A theme change restyles the TUI without a second setting
- [+] The first frame answers which deck, target, and providers are active
- [+] A new user reaches setup from the TUI instead of an empty list
- [-] Provider detection adds bounded filesystem reads before the first frame
- [-] Light surfaces are fixed values; a custom light palette cannot tune them yet

[HERDR]: https://github.com/herdrdev/herdr
