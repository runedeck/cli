---
title: "A proof set is reviewed on one page that plays the committed recording"
description: "rune proof page writes one self-contained page per proof set with scene chapters, scenario text, a bench-protocol judgement, and owner verdicts, and the page hashes the committed transcript"
type: adr
category: architecture
tags:
    - cli
    - proof
    - review
status: proposed
created: 2026-10-07
updated: 2026-10-07
author: "@N4M3Z"
project: cli
related:
    - "CLI-0022 Native bench runner"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream:
    - "one-page-reviews-proof-sets#one-page-plays-a-proof-set"
    - "https://github.com/runedeck/bench/blob/main/docs/reference/judged-evals.md"
change: one-page-reviews-proof-sets
---

# A proof set is reviewed on one page that plays the committed recording

## Context and Problem Statement

`rune proof run` records a scene per scenario and writes a cast, a transcript, and a README whose `transcript` field is the transcript's SHA-256. The owner reviews a proof set before signing, and the review needs the scenario beside the recording, one scene at a time, an independent judgement, and a place for the owner's verdict. A GIF per proof gives none of these, and a page that plays a re-timed copy of the cast cannot show that it plays what was committed.

## Considered Options

1. One HTML file per proof set, written by `rune proof page`, with the player inlined, the committed cast re-timed at load, and the transcript hashed in the browser against the README.
2. One page per proof built by the deck's AcceptanceTesting script around the html-tools player, with a GIF beside it.
3. A hosted viewer that reads the repository at a commit.

For the judgement: the bench judged-suite protocol, or a new verdict shape of its own.

## Decision Outcome

Option 1, with the bench protocol for the judgement. The page inlines `page.css` and `page.js` from the cli source, so a proof set needs no vendored player and no network. The cast events keep their content and the page re-times only their offsets, so a 0.1 s run reads as scenes and the digest badge still binds the page to the committed transcript. The judge writes the proof set as a judged suite and scores each THEN claim with the bench judge prompt, so a proof set reports like any judged suite.

### Consequences

- [+] One command gives the owner the review page for any proof set, in any repository that `rune proof run` serves.
- [+] The digest badge turns the page into a check: a page that plays an edited recording shows it.
- [+] The judgement reuses the bench contract, so its prompt, its parse rule, and its report live in one place.
- [-] The cli carries a stylesheet and a script. They are two files under `src/cli/proof/` with one owner.
- [-] The deck's `proof-page.py` and the GIF step become legacy for proof pages. The html-tools player keeps serving other pages, with the interval clock.
- [-] A judgement costs model calls: 177 criteria for ten proofs, about one Opus call each.
