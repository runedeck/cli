# Design

The full design, the adversarial pass, the proof contract, the hand partition, and the proof of the proof mechanism live in the workshop under `docs/specs/2026-09-24-rune-review/`. This file holds what the code needs.

## Modules

- `src/cli/closure/`: `Target` (WorkingCopy, Revisions { from, to }, Change(id)) resolves to `Resolved { head, base, parts, scenarios }`. `Part { name, role, paths, digest }`. The role map is a table per repository kind: the cli crate layout, or a deck's `deck.yaml` domains. Reading order and the deepest-directory split live here and nowhere else.
- `src/cli/proof/`: `Frontmatter { change, head, recorded, transcript, scenes }`, `Scene { scenario, kind, model }`, the README reader and writer, `scaffold`, `run`, `check`. The runner parses the fences itself in `fence.rs`, because trycmd's `TestCases::run` returns nothing and keeps outcomes private. It runs each command with the standard process API with stdout and stderr on one capture file (`runner.rs`), and matches output with its own `[..]` and `...` rules (`matcher.rs`), so every scene has an outcome the runner owns. Each scene gets its own temporary directory, a `$ cd` first line sets the working directory under the repository root, and `RUNE_PROOF_ROOT` names the repository for `sh -c` scenes. The cast is written from the captured chunks in asciinema v2.
- `src/cli/graph/lifecycle.rs`: `render_proofs` walks `docs/proofs/*/README.md`, keys scenes by the same `iri_fragment` the scenarios use, and emits `rune:Proof` with `proves` and `recordedWith`.
- `src/cli/review.rs`: `parts`, `open`, `diff`, `close`, and marker ingestion in `export`. `open` shells out to `jj workspace add -r <head> --sparse-patterns=empty`, `jj sparse set`, and `jj status` on the root. `close` reads `.review/REVIEW.md`, re-resolves, and writes the receipt.

## Data

- `REVIEW.md` is markdown with one YAML block: `target`, `head`, `base`, `parts` (name, role, paths, digest), `scenarios` (key, kind), `findings` (path, old_lines, kind, text), `verdict`, `reviewer`. The same record is written to the state file under the rune state directory, keyed by workspace name, and `close` re-resolves from the state file, then reads `verdict` and `findings` from the block.
- A part digest is the sha256 of the part's git-format diff for a diff target and of the part's file contents at the head for a closure, so a rewritten source under the same path changes it.
- The review receipt is the queue's receipt shape with `kind: review`, `target`, `head`, `base`, `parts`, `verdict`, `reviewer`, and ends with `review-exit=0` for `accept` and `review-exit=1` otherwise.

## Order of work

Group 1 is merged alone, because the deck's acceptance shape waits on the proof nodes. Group 2 is merged next and retires the bash driver. Group 3 is merged with the two reproductions as its proof. Evolution and operation-range targets, and the container runner, are separate changes.

## Markers

A marker is resolved by the author's later commit, never by the reviewer deleting it. `close` reads markers from every recorded path in the annotation commit, so the block in `REVIEW.md` is the reviewer's verdict and findings, and the markers are the line-bound findings. Both are inputs to `close`.
