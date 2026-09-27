---
adr: docs/changes/proof-scene-layout/adr.md
status: proposed
decisions: ["A proof cast is paced and framed for a reader"]
---

# Proof scene layout

## Why

`rune proof run` writes `proof.cast` from the captured output at real offsets. Twenty-three scenes of a fast command sit inside the first 0.2 seconds. No typed command appears, and the only marker between scenes is a blue rule with a `# Scenario:` line that carries the whole scenario key. A person who opens the GIF sees one frame of output and cannot tell which command produced it. The deck's fallback driver `prove.sh` has the layout the owner picked on 2026-09-25: a capability rule, a boxed `N/T · title`, a kind line, dim comments, a typed command behind a prompt, the output, one green tick per expectation, and a closing tally. That layout exists only in bash, so a proof recorded by rune and one recorded by the driver look different.

## What Changes

- The cast `rune proof run` writes is paced for a reader, not by the clock: a fixed pause after each scene (`--pause`, default 3 s), a shorter one after a header or a comment. Each command is typed word by word behind a prompt.
- Each scene opens with a boxed `N/T · title` and a kind line that names the requirement. Each capability opens with a rule that carries its name and scene count. The cast closes with a tally.
- A fence line `# text` before a command is a comment. The cast shows it dim and italic, and the transcript records it with the command lines.
- Each step ends with one tick that names the exit status and that the output matched; a failed step ends with a red cross and the first line of the reason.
- The transcript `proof.txt` and its digest change only by the comment lines a README adds.

## Capabilities

### New Capabilities

- proof-scene-layout

## Impact

- `src/cli/proof/fence.rs` (comment lines), `src/cli/proof/runner.rs` (per-step verdicts), `src/cli/proof/mod.rs` (the cast writer and pacing), `src/cli/mod.rs` (`--pause`).
- Every recorded `proof.cast` re-records to gain the layout. The transcripts hold unless a README adds comments.
- The deck's AcceptanceTesting skill drops `prove.sh` as the layout's home once this merges. The page tooling renders both proof and session casts in one frame vocabulary.
