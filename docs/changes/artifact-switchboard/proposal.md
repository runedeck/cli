---
adr: "docs/decisions/CLI-0032 Per-Provider Artifact Toggles.md"
status: proposed
---

# Artifact Switchboard

## Why

Turning one skill off for one harness needs a hand edit of `.rune` today. Consumers who run
several harnesses want different rune sets per harness from one command, with the state visible
in one view. Governing decision: CLI-0032.

## What Changes

- `.rune` gains per-provider `exclude` and optional `include` overlays per source.
- The kind commands gain toggle verbs: `rune skill off <Name> --provider claude`,
  `rune rule on <Name>`. Without `--provider` the toggle applies to every enabled provider.
- `rune <kind> list` shows a rune-by-provider state matrix.
- Assemble excludes toggled-off runes. Install prunes their deployed copies into the trash
  quarantine.
- The TUI gains a matrix editor over the same overlay data.

## Capabilities

- toggle (new)

## Impact

- `.rune` schema and its version step
- The kind commands, assemble filtering, install pruning, and the TUI editor
- The syntax-preserving manifest writes
