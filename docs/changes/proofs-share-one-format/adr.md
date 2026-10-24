---
title: "Proofs share one format, one record, and one view"
description: "A proof is a scrut Markdown test document that drives screens through tui-test, records evidence as in-toto statements, needs a judge and the owner for screen and model parts, and renders from one view model as text, CTRF JSON, and a dashboard page"
type: adr
category: architecture
tags:
    - cli
    - proof
    - provenance
    - review
status: proposed
created: 2026-10-22
updated: 2026-10-22
author: "@N4M3Z"
project: cli
related:
    - "Review and proof share one closure over the specification graph"
    - "CLI-0041 Signing Queue"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-opus-5-5"]
informed: []
upstream: []
change: proofs-share-one-format
---

# Proofs share one format, one record, and one view

## Context and Problem Statement

A proof shows that a commit behaves as a scenario states. The runedeck repositories record proofs in three shapes:

- Bash scripts in a driver grammar, with regex expectations and shared state.
- READMEs with one console fence per scenario, run by `rune proof run` in fresh directories.
- A YAML file of keystrokes that drives the Claude Code screen through tmux. A model's verdict and the owner's approval are written back into the same file.

Only the second shape has a specification, and none of the three can express what the others do. The evidence is bound in three different ways, and in two of them nothing checks it. Each reader (the proof pane, the spec pane, the review pane, and a throwaway gallery) parses the files its own way. The owner asked for one canonical format and one canonical view, built by wiring widely accepted tools and formats together.

## Considered Options

1. Extend the console fence grammar with shared state, regex lines, and screen steps. Keep the frontmatter record and add verdict fields.
2. Adopt an existing Markdown test-document format for every scene, drive screens with an existing terminal test tool from ordinary blocks, and record evidence as standard attestation statements.
3. Make every proof a script and let rune read its scenario lines.

## Decision Outcome

Option 2.

- **Source.** A proof MUST be one README per change in scrut's Markdown test format. It has a Setup section, one section per scenario key, and a Teardown section.
- **Live scenes.** A live scene MUST drive the screen with tui-test's command line from blocks under an "On screen" subheading.
- **Running.** Scenes MUST run as one chain in document order, in a fresh checkout of the head commit with an allowlisted environment.
- **Record.** Evidence MUST be bound in one in-toto statement file per proof. Each recorded part gets SLSA provenance and a test result. Each screen part and model prompt also gets a judge statement and an owner statement.
- **Status.** Status MUST be computed from digests alone. A pending scene fails the check.
- **View.** Every reader MUST read one view model that the check builds. Its machine form is CTRF JSON.

Option 1 keeps a grammar only rune knows and grows it by every feature the other two shapes have, so the custom parser and matcher grow instead of retiring. Option 3 gives up the Markdown document that a reviewer reads and the graph indexes.

## Consequences

- One grammar, one record, and one view replace three of each. The new code is not smaller than the code it retires. The design estimates two to three thousand lines of Rust against about 2,300 retired lines of Rust and script, counted with `wc -l` on main of 24 October 2026.
- Dependencies join. rune links the scrut library pinned at 0.4.3, the last version on crates.io. scrut 0.5 changes the library interface. The tui-test release binary and asciinema-player are outside tools, pinned by version and digest. If scrut fails the spike, the fallback is option 1 with the same record and view.
- A rewritten head no longer drops judgments and approvals. The record binds file digests and the head's change id, and head drift is a warning that only the archiver enforces.
- Recording a screen or a model prompt leaves the earlier judgment and approval stale by digest, with no code to clear them. This matches the pending-snapshot review of insta.
- Committed GIFs leave the repositories: 47.7 MiB in mods. A pull request that needs motion gets a GIF rendered from a recording.
- The paced recording of command scenes from proof-cast-reads-scene-by-scene stays. Its writer reads scrut's results instead of the console runner's. It is a render, so its digest is recorded and no status depends on it.
- The review predicate is custom, because no published in-toto predicate covers a human approval. The test-result predicate is used with the evidence files as its subject instead of the tested source. That is a stated departure from its specification.
- The record is unsigned. Signed commits and the merge seal authenticate it. DSSE envelopes wait until a record leaves the repository.
