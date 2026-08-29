---
title: "Deck Discovery"
description: "rune discover lists community decks through one GitHub topic search, read-only and bounded"
type: adr
category: cli
tags:
    - cli
    - discovery
    - community
status: proposed
created: 2026-08-29
updated: 2026-08-29
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0032 Per-Provider Artifact Toggles"
    - "CLI-0035 Plugin Manifests"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5"]
informed: []
upstream: []
---

# Deck Discovery

## Context and Problem Statement

A consumer finds decks only by word of mouth. The switchboard makes single runes cheap to try,
but nothing lists what the community publishes. [Herdr][HERDR] lists community plugins through a
GitHub topic: a publisher tags the repository, the tool searches the topic. No registry service,
no submission queue.

## Decision Drivers

- Zero-infrastructure listing: publishing is one repository topic
- Read-only and bounded: one request, one timeout, no token requirement
- The result must hand the user the exact next command

## Considered Options

1. **Central registry service** — a curated index with submissions. Infrastructure, moderation,
   and a single point of failure before the community exists.
2. **Curated list file in the cli repository** — simple, but every listing needs a pull request
   and the list goes stale.
3. **GitHub topic search** — publishers tag their repository `runedeck-deck`; `rune discover`
   searches the topic.

## Decision Outcome

Option 3. `rune discover [QUERY] [--json]` searches public repositories carrying the
`runedeck-deck` topic through the GitHub search API, unauthenticated, with a ten-second timeout.
Each row shows the name, description, stars, and URL, plus the exact staging command shape.
Failures are structured with a diagnosis fix command; a rate-limit response names the wait.
Listing quality stays with the publishers; rune never executes or clones anything during
discovery.

## Consequences

- [+] Publishing a deck is one repository topic, no gatekeeper
- [+] Discovery pairs with the switchboard: find, add, toggle
- [-] Unauthenticated search is rate-limited; heavy use needs a later token path
- [-] The topic namespace is unmoderated, so listings carry no endorsement

[HERDR]: https://github.com/herdrdev/herdr
