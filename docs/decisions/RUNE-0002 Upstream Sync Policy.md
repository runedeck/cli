---
title: "Upstream Sync Policy"
description: "Predecessor deltas port as content with staged commits; deck semantics win on conflicts."
type: adr
category: cli
tags:
    - upstream
    - porting
    - rename
status: accepted
created: 2026-07-13
updated: 2026-07-13
author: "Grok (grok-composer-2.5-fast)"
project: rune
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["HarnessCouncil"]
informed: []
upstream: []
---

# Upstream Sync Policy

## Context and Problem Statement

The predecessor CLI repository keeps evolving after rune's fresh-root fork. Cherry-picking fetched upstream commits is technically possible without shared ancestry, but the results carry upstream naming and bypass the review and rename discipline rune requires. The team still needs bug fixes and features from upstream. Uncontrolled copying would reintroduce predecessor naming and semantics that conflict with the deck model.

## Considered Options

- Cherry-pick fetched upstream commits directly, accepting their naming and shape
- Port deltas as content, staged by subsystem, with the rename applied on import
- Freeze at the fork point and reimplement upstream features independently

## Decision Outcome

rune ports predecessor deltas as content, never as cherry-picks.

Port workflow:

- Stage work by subsystem with one commit per subsystem on import.
- Apply the rune rename on import.
- Finish with a rename sweep: every surviving legacy string must be a deliberate compatibility affordance documented in code or ADR, not an accident.

Where an upstream change and rune's deck-model semantics touch the same code, deck semantics win and the upstream feature adapts to rune's model.

Each sync ends with the full check suite. Newer upstream commits that land mid-port queue as follow-up ports rather than restarting the active port.

## Consequences

- rune absorbs predecessor fixes without re-merging shed history
- Staged commits keep ports reviewable and bisectable
- Deck-first conflict resolution prevents semantic regression but may delay or reshape upstream features that assume predecessor behavior
- Mid-port upstream arrivals wait in queue, which avoids thrash but can leave rune briefly behind the predecessor tip

