# Tasks

## 1. Closure and graph

- [ ] 1.1 `src/cli/closure/`: a change resolves to proposal, delta specs in `### New Capabilities` order, scenario keys as the exporter mints them, records in `decisions:` order, ideas, proofs by frontmatter `change`
- [ ] 1.2 Proof frontmatter reader with validation errors that name the field. Tested on the four deck proofs that carry it
- [ ] 1.3 `graph/lifecycle.rs` emits `rune:Proof`, `rune:proves` to the `head` commit and to proven scenes only when `proof.txt` matches `transcript`, `rune:recordedWith` on instruction scenes
- [ ] 1.4 `rune spec validate` rejects a malformed proof README
- [ ] 1.5 Tests: closure of a fixture change, unplaced path, proof resolved by `change` not by directory, export shows four proofs and no edge for `unproven`

## 2. Proof runner

- [ ] 2.1 `rune proof scaffold <change>`: README with frontmatter, one empty fence per scenario key in closure order, refuses an existing README
- [ ] 2.2 `rune proof run <change>`: own fence parser in the trycmd grammar, execution through snapbox with `code` and `stdout_eq`, `rune` resolved to its own path and the rest through `PATH`, a fresh directory per scene, `$ cd` honored, unresolvable command fails the scene, one line per scene
- [ ] 2.3 Run writes `proof.txt` with one section per scene, `transcript`, `head`, `recorded`, and each scene's kind, failed scenes `unproven`. Instruction scenes skipped, recorded by `run --instruction <key>` through `rune run`. Cast through `asciinema`, else `scripts/fallback/asciinema.py`, else `docs/proofs/cast.py`, when one answers
- [ ] 2.4 `run --check` rebuilds the expected transcript, refuses a digest mismatch and a `head` that is not an ancestor
- [ ] 2.5 `docs/proofs/review-and-proof/README.md` holds a scene for every scenario of this change and passes `rune proof run`
- [ ] 2.6 Tests: fence grammar, unregistered command, output mismatch diff, drifted transcript, recorder absent

## 3. Review commands

- [ ] 3.1 `rune review parts <target> [--json]` for the working copy, a revset or `--from`/`--to`, and a change id. Role map for the cli crate and for a deck
- [ ] 3.2 `rune review open`: workspace at the head, sparse union, root snapshot for the working copy, `.review/REVIEW.md` plus the state file, printed invocations and recoveries, `--no-generated`, merges and multi-commit revsets refused
- [ ] 3.3 `rune review diff`: the grouped git-format patch
- [ ] 3.4 `rune review close`: re-resolve from the state file, digest check, every recorded path scanned in the annotation commit, `accept` refused on an open `[ISSUE]` or an issue finding, receipt ending `review-exit=0` only for `accept`
- [ ] 3.5 `rune review export` ingests file markers with `.rune-comments.yaml`
- [ ] 3.6 Reproductions: `parts core-foundation-principles` on the deck equals the canvas A sparse set. `parts 9dd8965f` on cli equals the hand partition of ten parts
- [ ] 3.7 Tests: three targets, partition rules, stale result, accept with an issue, changes keeps markers, unanchored finding

## 4. Verification

- [ ] 4.1 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, full suite, `rune spec validate review-and-proof`, `rune docs check`
- [ ] 4.2 Adversarial review by astra after each of groups 1, 2, and 3, findings and dispositions in the workshop under `docs/specs/2026-09-24-rune-review/`
- [ ] 4.3 Deck: `rune proof scaffold core-foundation-principles` runs, the deck fills scenes, `prove.sh` retires. `.review/` in the deck's lint excludes
- [ ] 4.4 CHANGELOG entries under Unreleased, `rune` help text, `AGENTS.md` rows

## 5. Record

- [ ] 5.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
