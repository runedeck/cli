---
title: "Drafts live in the consumer and promote into the deck"
description: "A rune under construction lives unmanaged in the consumer's provider tree, tracked by a .drafts register, and enters the deck only through promote."
type: adr
category: architecture
tags:
    - cli
    - drafts
    - lifecycle
status: proposed
created: 2026-09-20
updated: 2026-09-20
author: "@N4M3Z"
project: cli
related:
    - "CLI-0003 Conflict Resolution on Install"
    - "ASSEMBLY-0003 Manifest-Based Deployment Tracking"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: draft-runes-before-promotion
---

# Drafts live in the consumer and promote into the deck

## Context and Problem Statement

The deck's change lifecycle is right for a rune that stays and wrong for the first hour of one. A hand-written file in the provider tree is an orphan to `rune doctor`, and repair removes it. The author needs a fast path that the tools understand.

## Considered Options

1. A draft flag in the deck: the file sits in `runes/<domain>/` with `status: draft` and installs with a marker.
2. An unmanaged file in the consumer, tracked by a register beside the manifest, with a promote command into the deck.
3. A drafts directory in the consumer, linked into every provider tree at install.

## Decision Outcome

Option 2. A draft MUST live in the consumer's provider tree, outside `.manifest`, and MUST be listed in `.drafts`. `rune doctor` MUST treat a registered draft as neither orphan nor drift and MUST report its age. `rune promote` MUST be the only path from a draft into the deck, and it MUST create the change stub, so the lifecycle starts at once.

## Consequences

- The harness loads a draft the moment it is written. No install, no change, no review.
- The manifest stays the record of managed files. A second small file carries the drafts.
- A draft that is never promoted stays a draft. Doctor reports its age, and nothing removes it. The owner decides.
- A consumer with drafts is not reproducible from the deck until promote runs. That is the point of the register: doctor says so.
