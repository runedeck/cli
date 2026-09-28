---
title: "A proof cast is paced and framed for a reader"
description: "rune proof run writes the cast on a presentation timeline with one frame vocabulary per scene, so a person can read a GIF and a page shows the same scene a transcript records"
type: adr
category: architecture
tags:
    - cli
    - proof
    - cast
status: proposed
created: 2026-09-27
updated: 2026-09-27
author: "@N4M3Z"
project: cli
related:
    - "DECK-0015 Behavior Proof per Change"
    - "DECK-0019 A Proof Plays as a Cast Where a Person Reads It"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1", "gpt-6-astra"]
informed: []
upstream:
    - "https://docs.asciinema.org/manual/asciicast/v2/"
change: proof-scene-layout
---

# A proof cast is paced and framed for a reader

## Context and Problem Statement

A proof has two readers. The transcript binds the claim and is read by `rune proof run --check` and by the graph. The cast is read by a person, on a page or as a GIF, and the person decides whether to sign. `rune proof run` writes the cast at real offsets, so a proof of fast commands is one frame of output with no typed command, and the deck's bash driver holds the layout the owner picked. Which timeline and which frame vocabulary should the cast carry?

## Considered Options

1. Real time. The cast shows what the clock saw. Honest about duration, unreadable for fast commands, and different from the driver's layout.
2. Presentation time with the driver's layout. The cast writes fixed pauses and types each command, one frame vocabulary for a proof and for a session recording, and the transcript alone binds the claim.
3. Post-process the real-time cast in the deck. The layout lives in a second tool, every proof needs two steps, and a page can show a cast the run never wrote.

## Decision Outcome

Option 2. The cast MUST be paced for a reader: a fixed pause after each scene (`--pause`, default 3 s), a shorter one after a header or a comment, and each command typed word by word behind a prompt. Each scene MUST open with a boxed `N/T · title` and a kind line naming the requirement, each capability with a rule naming it and its scene count, and the cast MUST close with a tally. Each step MUST end with one tick that names the exit status and the match, or a red cross with the reason. A fence comment `# text` MUST show dim and italic before its command and MUST be part of the transcript. The transcript stays the record. The cast's offsets are a reading aid and never evidence of duration.

Option 1 keeps the reader out. Option 3 splits the layout from the run and lets a page play a cast that differs from the transcript.

### Consequences

- [+] One frame vocabulary for a proof, a session recording, and the page's scene list, in the layout the owner already chose.
- [+] A GIF of a fast proof is readable without the page.
- [-] The cast no longer tells how long a command took. A proof that wants duration on record puts it in the output.
- [-] Every recorded cast re-records to gain the layout, in the deck and in the cli.
- [-] Comments in a fence join the transcript, so adding one to a recorded proof changes its digest and needs a re-run.
