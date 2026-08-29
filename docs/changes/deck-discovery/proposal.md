---
adr: "docs/decisions/CLI-0033 Deck Discovery.md"
status: proposed
---

# Deck Discovery

## Why

A consumer finds decks only by word of mouth. The switchboard makes single runes cheap to try,
but nothing lists what the community publishes. Governing decision: CLI-0033.

## What Changes

- `rune discover [QUERY] [--json]` lists public repositories carrying the `runedeck-deck`
  GitHub topic: name, description, stars, URL, and the exact staging command.
- One unauthenticated request, a ten-second timeout, structured failures, no writes.

## Capabilities

- discover (new)

## Impact

- One new CLI module and its dispatch route
- Documentation: publishing a deck means adding one repository topic
