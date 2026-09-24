---
adr: docs/changes/review-and-proof/adr.md
status: proposed
decisions: ["Review and proof share one closure over the specification graph"]
---

# Review and proof

## Why

A commit is no longer the unit a reviewer wants. Agents make many commits, rewrite them, and merge work that spans records, delta specs, code, tests, and proofs, so the owner reviews the closure of a change, not a diff of one commit. The deck proved the workspace by hand on 2026-09-23: a jj workspace under `.workspaces/review-canvas-a`, sparse to the fourteen patterns that select the 27 files of `core-foundation-principles`, opened in Zed, commented in place. Nothing builds that workspace, nothing lists the scenarios it holds, and nothing records what was reviewed against which revision.

Proofs have the same gap. A proof is a bash script in the deck's driver grammar, recorded by hand, and the graph has zero `rune:Proof` nodes, so a record can be accepted with unproven scenarios. Canvas A has 41 scenarios and no scene for any of them.

## What Changes

- `rune proof scaffold <change>` writes `docs/proofs/<change>/README.md` with the proof frontmatter and one empty trycmd console fence under each scenario key, in closure order, every scene `kind: unproven`.
- `rune proof run <change>` parses the fences in the trycmd grammar (`$ command`, `? <status>`, `[..]`, `...`), executes them through snapbox, records the cast when a recorder answers, writes `proof.txt` with one section per scene and its sha256 and the head commit into the frontmatter, and sets each scene's kind. A failed or unresolvable scene is `unproven`.
- `rune graph export` emits a `rune:Proof` node per proof README with `rune:proves` edges to the commit and to each proven scenario.
- `rune review parts`, `open`, `diff`, and `close` resolve a target (the working copy, a revision comparison, or a change directory) to an immutable head, a base, and ordered parts. `open` builds a sparse jj workspace at the head and writes `.review/REVIEW.md` with the parts, the reading links, and every scenario with its scene kind. `close` binds the result to head, base, and per-part digests and writes it as a receipt beside the checks receipts.
- `rune review export` reads the `[ISSUE]`, `[NOTE]`, `[SUGGESTION]`, and `[PRAISE]` markers in files as well as `.rune-comments.yaml`.

## Capabilities

- proof-scene-runner (new)
- sparse-review-workspace (new)

## Impact

- New modules `src/cli/closure/` and `src/cli/proof/`. `src/cli/review.rs` grows the four subcommands, `src/cli/graph/lifecycle.rs` gains the proof emitter, and `snapbox` joins the dependencies.
- The deck retires `scripts/fallback/prove.sh` once `rune proof run` is merged and adds `.review/` to its lint excludes. The skeleton carries the template copies.
- Proofs under `docs/proofs/` migrate from `record.sh` scripts to README scenes, this change's own first.
