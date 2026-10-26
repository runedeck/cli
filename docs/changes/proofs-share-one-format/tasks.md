# Tasks

## 1. Spike

- [x] 1.1 The owner installs the scrut 0.4.3 binary and a pinned tui-test release binary on the host, and records both versions and digests in the design
- [x] 1.2 scrut library: rune's frontmatter keys are ignored, heredocs work in `>` continuation lines, combined output keeps the order of stdout and stderr, `* (glob*)` matches as used, `cd` persists between blocks, the chain stops at the first failure, the library sets none of the binary's variables, and the parser, executor, and Markdown generator are public
- [x] 1.3 tui-test against Claude Code on one mods live scene: the recording format version, no-break spaces under `--regex`, `submit` reads as typed input, the daemon's environment for `claude`, the ready marker, the trust dialog, and `--strict-mcp-config` with `--plugin-dir`
- [x] 1.4 Write the spike's findings into `spike.md`. If scrut fails, stop and bring the ADR's option 1 back to the owner

## 2. Records

- [x] 2.1 Move every-scenario-has-proof out of review-changes-with-proofs: delete its delta there, update that proposal's capabilities, and mark its ADR's proof-format decision as replaced by this change's ADR
- [ ] 2.2 Ontology: the `live` kind, and the terms judgment, approval, recording, record, and screen text, with glossary pages. Redefine attestation and `transcriptMatches`
- [ ] 2.3 `rune spec validate proofs-share-one-format` and `rune docs check` pass
- [x] 2.4 A scene in this change's proof names `console-fences-prove-scenarios#proof-frontmatter-is-the-record/instruction-scene-without-a-model` over `tests/fixtures/proof/bad-instruction`
- [x] 2.5 A scene names `console-fences-prove-scenarios#run-executes-every-executable-scene/scene-names-a-command-the-runner-cannot-find`
- [x] 2.6 A scene names `console-fences-prove-scenarios#a-step-reads-its-input-from-the-fence/step-feeds-a-script-through-standard-input`
- [x] 2.7 A scene names `console-fences-prove-scenarios#a-step-reads-its-input-from-the-fence/input-line-before-any-command` over `tests/fixtures/proof/input-first`
- [x] 2.8 A scene names `console-fences-prove-scenarios#every-scene-starts-clean/scenes-do-not-share-a-directory` over `tests/fixtures/proof/isolation`
- [x] 2.9 A scene names `console-fences-prove-scenarios#the-transcript-binds-the-claim/scene-key-edited-after-the-run` over `tests/fixtures/proof/edited-key`
- [x] 2.10 A scene names `console-fences-prove-scenarios#the-transcript-binds-the-claim/instruction-scene-never-run` over `tests/fixtures/proof/instruction-pending`
- [x] 2.11 A scene names `console-fences-prove-scenarios#check-refuses-drift/transcript-drifted-after-the-run` over `tests/fixtures/proof/drifted`
- [x] 2.12 A scene names `console-fences-prove-scenarios#the-run-records-its-own-cast/cast-written-from-the-captured-output` and reads the cast header
- [x] 2.13 A scene names `console-fences-prove-scenarios#the-graph-shows-only-proven-scenes/proof-transcript-does-not-match` through `rune graph export`

## 3. Command scenes

- [ ] 3.1 scrut 0.4.3 joins as a pinned dependency. The adapter splits sections with pulldown-cmark and refuses a README whose block boundaries differ from scrut's parser
- [ ] 3.2 Checkout of head at a stable cache path (jj workspace, else git worktree), removed after the run. Allowlisted environment, `RUNE_PROOF_ROOT`, `RUNE_SCENE_DIR`, scrut's variables with `COLUMNS=80`, `NO_COLOR=1`. Keep today's isolation: a fresh `RUNE_STATE_DIR` for each chain with `XDG_STATE_HOME` removed, and the `RUNE_PROOF_SCENE` marker (runner process id and HOME), so `rune install` inside a scene skips the hook step and reports `hooks.skipped`
- [ ] 3.3 One chain in document order, Teardown always, `--scene` replay without recording, `needs` checks with the skip reason
- [ ] 3.4 Transcripts through scrut's Markdown generator, scene folders, `files/`, the line diff on a failure
- [ ] 3.5 `rune proof scaffold` writes Setup, the scenario sections, and Teardown
- [ ] 3.6 Tests ported from today's runner tests, plus chain, replay, skip, checkout, and environment tests

## 4. Record

- [ ] 4.1 One generic serde set of Statement, ResourceDescriptor, and SLSA structs, used by the sidecar reader, the adoption provenance, and the proof record
- [ ] 4.2 Section digests, the bundle reader and writer, the rewrite rule that keeps other lines
- [ ] 4.3 Status from digests, head drift by change id or ancestry, the archiver's refusal
- [ ] 4.4 Graph exporter: proves edges from the status
- [ ] 4.5 Pages for the `proof/v1` build type and the `review/v1` predicate under `docs/`
- [x] 4.6 Today's check holds a signed or reworded head: `rune proof run` records `head_change` and `head_tree`, `--check` accepts an ancestor with both, and a proof without them reads both from the old commit. Tests in `tests/proof_head.rs` cover a signed, a reworded, and a content-edited head

## 5. View

- [ ] 5.1 `ProofView` in `src/view.rs`, its text form, and CTRF JSON. A CTRF fixture the mods pane tests share
- [ ] 5.2 `rune proof check [<change> | --all]` fails on stale, failed, or pending, and `--rerun` runs the command blocks in a fresh checkout and writes nothing
- [ ] 5.3 Dashboard `/proofs` with asciinema-player pinned, grouped by repository and change
- [ ] 5.4 GIF cache keyed by the recording digest, and the validator's report of a committed image
- [ ] 5.5 `src/cli/proof/cast.rs` renders the paced recording from scrut's results, with the comment paragraph above a block shown dim, and the record names it as a byproduct

## 6. Live scenes and judging

- [ ] 6.1 Screen steps: the probe session, the daemon outside the chain and under the chain's allowlisted environment (`spike.md` shows a harness session's token reaching the program otherwise), the startup marker, `record start` and `record stop` around each scene's screen steps, screen text
- [ ] 6.2 Model prompts through `rune run` from Setup alone, on request only
- [ ] 6.3 `rune proof judge`: the `proof.models` trusted list, the other-vendor rule, answer-only mode, every input in the prompt, the raw reply and the judgment
- [ ] 6.4 `rune proof approve`: the review loop after `cargo insta review`, accept, reject, overrule, refusal without a terminal

## 7. Migration

- [ ] 7.1 `rune proof convert` with the rules in `migration.md`
- [ ] 7.2 Convert and record the cli frontmatter proofs, and re-specify review-changes-with-proofs' proof
- [ ] 7.3 Map, convert, and record the six cli bash proofs. Delete `driver.sh`, `cast.py`, and `signing-fixture.sh` once nothing sources them
- [ ] 7.4 Convert the four run proofs recorded in the old form (run-works-inside-a-harness, run-returns-harness-artifacts, run-isolates-write-runs, run-writes-results-and-agy-reads), turning each profile line into a declared need
- [ ] 7.5 Delete `fence.rs`, `matcher.rs`, and the converter once no console proof remains in cli or mods

## 8. Verification

- [ ] 8.1 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, the full test suite, `rune spec validate`, `rune docs check`
- [ ] 8.2 This change's own proof records every scenario and edge case, and `rune proof check --all --rerun` passes in CI
- [ ] 8.3 A review by a model of another vendor after groups 3, 4, and 6, with findings and dispositions in the change folder
- [ ] 8.4 CHANGELOG entries under Unreleased, help text, and `AGENTS.md` rows

## 9. Record

- [ ] 9.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
