---
adr: docs/changes/proofs-share-one-format/adr.md
status: proposed
decisions: ["Proofs share one format, one record, and one view"]
---

# Proofs share one format

## Why

Behavior proofs exist in three shapes across the runedeck repositories, and only one of them is a format the proof runner knows.

- Six cli proofs are bash scripts in the driver grammar, with regex expectations and state shared across scenes. Their READMEs pin a transcript digest that no code checks.
- Three cli proofs and seventeen mods proofs are READMEs with frontmatter and one console fence per scenario. The proof runner executes them, but every scene starts in a fresh directory, so a proof cannot tell a story. None of the three cli proofs passes the check today.
- Five mods proofs drive the Claude Code screen from a hand-written YAML file through tmux. A model judges the captured screens and the owner approves them, through four scripts that no specification defines. Three of these READMEs put the recording's hash where the transcript digest belongs, so the check fails on them.

There is no common view either. The proof pane plays only one GIF per proof and ignores live proofs. A throwaway gallery was built by hand to see all 29 proofs at once. The owner asked on 22 October 2026 for one canonical way to record proofs and one canonical way to view them, built from widely accepted formats.

## What Changes

- A proof is one README per change, written in scrut's Markdown test format: a Setup section, one section per scenario, and a Teardown section. A live scene adds screen blocks under an "On screen" subheading. They drive the program through Microsoft's tui-test command line. The console fence grammar, the bash driver, and the live YAML file retire.
- Scenes run as one chain in document order, in a fresh checkout of the head commit, with an allowlisted environment. A scene may declare that it needs the owner's host or a harness.
- Each scene writes a transcript in the same Markdown format. A live scene also writes its screen text and a recording. The command scenes keep one paced recording per proof, in the layout of proof-cast-reads-scene-by-scene, rendered from the run. Animated images are rendered on demand into a cache and are never committed.
- Each proof keeps one record: a file of in-toto statements. Each recorded part gets a SLSA provenance statement and a test-result statement, and judge and owner statements name the digests of the files they read. Status (proven, stale, failed, pending, unproven) is computed from digests only.
- Screen parts and model prompts need a judgment from a model of another vendor and the owner's approval. A pending scene fails the check, so CI blocks until both exist.
- One view model feeds every reader. The check prints it as text or as CTRF JSON for the panes and CI. CI re-runs the command blocks. A proofs page in the dashboard plays the recordings.
- A one-time converter moves the console-fence proofs to the new format. The bash and live proofs are converted by hand.

## Capabilities

### New Capabilities

- `every-scenario-has-proof`: the proof source, the chain, the checkout of head, preconditions, transcripts, and the converter. This capability moves here from review-changes-with-proofs and is rewritten.
- `console-fences-prove-scenarios`: the proof runner in the code today, with main's scenario titles, so a proof can name its scenes until the converter retires them.
- `proof-record-binds-evidence`: the record, what each statement binds, status from digests, and head drift.
- `live-scenes-need-approval`: screen blocks, model prompts, the judge, and the owner's approval.
- `proofs-render-one-view`: the view model, its text and JSON forms, the CI re-run, the dashboard page, the paced recording of command scenes, and images on demand.

## Impact

- `src/cli/proof/`: the fence parser and the matcher retire. A scrut adapter, the record, the check, the judge, and the approval loop replace them. `src/view.rs` gains the proof view, and `src/cli/dashboard/` gains a route.
- New dependencies: the scrut library, pinned. The tui-test release binary and asciinema-player join as outside tools, pinned by version and digest.
- `src/services/sidecar.rs` and `src/manifest/provenance.rs` share one generic set of statement structs with the proof record.
- review-changes-with-proofs loses the every-scenario-has-proof delta, and its ADR's proof-format decision is replaced by this change's ADR.
- proof-cast-reads-scene-by-scene keeps its layout and pacing. Its writer reads scrut's results, and its `$#` comment line becomes a one-line paragraph above a block.
- Every proof under `docs/proofs/` in cli and mods migrates. The mods panes, CI, and proof scripts change in the mods change `proof-pane-reads-the-view`.
