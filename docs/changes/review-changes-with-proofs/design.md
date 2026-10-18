# Design

The full design, the adversarial pass, the proof contract, the hand partition, and the proof of the proof mechanism live in the workshop under `docs/specs/2026-09-24-rune-review/`. This file holds what the code needs.

## Modules

- `src/cli/closure/`: `Target` (WorkingCopy, Revisions { from, to }, Change(id)) resolves to `Resolved { head, base, parts, scenarios }`. `Part { name, role, paths, digest }`. The role map is a table per repository kind: the cli crate layout, or a deck's `deck.yaml` domains. Reading order and the deepest-directory split live here and nowhere else.
- `src/cli/proof/`: `Frontmatter { change, head, recorded, transcript, scenes }`, `Scene { scenario, kind, model }`, the README reader and writer, `scaffold`, `run`, `check`. The runner parses the fences itself in `fence.rs`, because trycmd's `TestCases::run` returns nothing and keeps outcomes private. It runs each command with the standard process API with stdout and stderr on one capture file (`runner.rs`), and matches output with its own `[..]` and `...` rules (`matcher.rs`), so every scene has an outcome the runner owns. A `< text` line after the `$` and `>` lines feeds the step's standard input from a file the runner writes beside the capture, so a script or a brief lives in the fence. Each scene gets its own temporary directory, a `$ cd` first line sets the working directory under the repository root, and `RUNE_PROOF_ROOT` names the repository for `sh -c` scenes. The cast is written from the captured chunks in asciinema v2.
- `src/cli/graph/lifecycle.rs`: `render_proofs` walks `docs/proofs/*/README.md`, keys scenes by the same `iri_fragment` the scenarios use, and emits `rune:Proof` with `proves` and `recordedWith`.
- `src/cli/review.rs`: `parts`, `open`, `diff`, `close`, and marker ingestion in `export`. `open` shells out to `jj workspace add -r <head> --sparse-patterns=empty`, `jj sparse set`, and `jj status` on the root. `close` reads `.review/REVIEW.md`, re-resolves, and writes the receipt.

## Data

- `REVIEW.md` is markdown with one YAML block: `target`, `head`, `base`, `parts` (name, role, paths, digest), `scenarios` (key, kind), `findings` (path, old_lines, kind, text), `verdict`, `reviewer`. The same record is written to the state file under the rune state directory, keyed by workspace name, and `close` re-resolves from the state file, then reads `verdict` and `findings` from the block.
- A part digest is the sha256 of the part's git-format diff for a diff target and of the part's file contents at the head for a closure, so a rewritten source under the same path changes it.
- The review receipt is the queue's receipt shape with `kind: review`, `target`, `head`, `base`, `parts`, `verdict`, `reviewer`, and ends with `review-exit=0` for `accept` and `review-exit=1` otherwise.

## Order of work

Group 1 is merged alone, because the deck repository's acceptance shape waits on the proof nodes. Group 2 is merged next and retires the bash driver. Group 3 is merged with the two reproductions as its proof. Evolution and operation-range targets, and the container runner, are separate changes.

## Markers

A marker is resolved by the author's later commit, never by the reviewer deleting it. `close` reads markers from every recorded path in the annotation commit, so the block in `REVIEW.md` is the reviewer's verdict and findings, and the markers are the line-bound findings. Both are inputs to `close`.

## Commands

The requirements name roles and behavior. This table holds every command, flag, environment variable, path, label, and exit status that implements a requirement. The roles and the commands that fill them today:

- validator: `rune spec validate` for the proof frontmatter, `rune proof run --check` for drift, and every `rune review` subcommand. `rune review parts` resolves a target through `src/cli/closure/`, `rune review open` and `rune review diff` open a review, and `rune review close` and `rune review export` close one.
- proof runner: `rune proof scaffold <change>`, `rune proof run <change>`, and `rune proof run --instruction <key>`, which records an instruction scene through `rune run <model>`. It resolves a change through `src/cli/closure/` too.
- generator: `rune graph export` writes the graph.

| Requirement | Command and flags | Hook | Diagnostic ids and literals | Exit status |
|---|---|---|---|---|
| Proof Frontmatter Is The Record | `rune spec validate`. The proof sits at `docs/proofs/<directory>/README.md`. The scenario key is `<capability>#<requirement-slug>/<scenario-slug>` | none | the validation error names the field | nonzero on a malformed proof |
| Head And Transcript Bind The Record | `rune spec validate`. The transcript file is `proof.txt` | none | none | 0 when `head` is absent and the rest is valid |
| One Scene Per Scenario Is Written First | `rune proof scaffold <change>` | none | the refusal names the existing README, or says the change has no scenario | nonzero on refusal, nothing written |
| Run Executes Every Executable Scene | `rune proof run <change>`. Fence grammar: `$ command` runs a command, a line that starts with `>` continues it, `? <status>` is the expected exit code, `[..]` elides within a line, and `...` elides lines. `rune` resolves to the runner's own path and every other command through `PATH` | none | one line per scene with key and outcome. A failure names the scenario key and the command | nonzero when a scene fails |
| A Step Reads Its Input From The Fence | `rune proof run <change>` and `rune proof run --check`. Input lines have the form `< text` | none | a `<` line before any command is a parse error that names the line | nonzero on the parse error |
| Every Scene Starts Clean | `rune proof run <change>`. `$ cd <path>` as the first fence line. `RUNE_PROOF_ROOT` is exported for `sh -c` scenes | none | none | n/a |
| The Transcript Binds The Claim | `rune proof run <change>` writes `proof.txt`. `rune proof run --instruction <key>` records an instruction scene through `rune run <model>` | none | none | n/a |
| Check Refuses Drift | `rune proof run --check` | none | names the recorded and the computed digest, or a `head` that is not an ancestor | nonzero on drift or a non-ancestor head |
| The Run Records Its Own Cast | `rune proof run <change>` writes `proof.cast`. `asciinema` re-records by hand and `agg` renders a GIF. Fallback recorders: `scripts/fallback/asciinema.py` and `docs/proofs/cast.py` | none | none | n/a |
| The Graph Shows Only Proven Scenes | `rune graph export`. Nodes `rune:Proof` and `rune:scene`, edges `rune:proves` and `rune:recordedWith`. A `# Scenario:` heading line also matches a scene | none | none | n/a |
| A Target Resolves To One Head, One Base, And Parts | `rune review parts <target> [--json]` | none | last part `unplaced` | nonzero on an ambiguous target |
| Three Targets Exist | No argument for the working copy. `--from <rev> --to <rev>` for a comparison. `<change-id>` for a closure | none | the refusal names the ambiguous revset or both parents | nonzero on refusal |
| A Change Declares Its Closure | `rune review parts <change-id>`. Ideas live under `docs/ideas`. The hand-built workspace is `.workspaces/review-canvas-a` | none | none | n/a |
| Parts Partition A Diff By Role | `rune review parts --from <rev> --to <rev>`. The role map is a table per repository kind: the cli crate layout, or a deck's `deck.yaml` domains | none | none | n/a |
| Reading Links Come From Declarations | `rune review open` writes the links to `.review/REVIEW.md` | none | none | n/a |
| Parts Read In One Order | `rune review open --no-generated` | none | none | n/a |
| The Workspace Is A Checkout Of The Head | `rune review open` runs `jj workspace add -r <head> --sparse-patterns=empty`, `jj sparse set`, and `jj status` on the root. The workspace is `.workspaces/review-<name>` and the review directory is `.review/` | none | the refusal names the existing workspace | nonzero on refusal |
| Open Records What It Resolved | `.review/REVIEW.md` in the workspace. The state file sits under the rune state directory, keyed by workspace name | none | none | n/a |
| Diff Prints The Grouped Patch | `rune review diff <name>` | none | none | 0 |
| Open Prints The Way In | Printed lines: `tuicr -r '<base>..<head>'`, `jj workspace update-stale`, and `jj abandon` of the copy without the reviewer's edits | none | none | n/a |
| Close Re-Resolves From The State File | `rune review close <name> --verdict accept\|changes\|reject` | the guarded push proves the reviewed tree | the refusal names the part | nonzero on refusal, no receipt |
| Only An Acceptance Is A Receipt | The receipt sits beside the checks receipts and `rune sign queue` reads it. Its last line is `review-exit=0` for `accept` and `review-exit=1` otherwise | none | the `review-exit` line | 0 when the receipt is written |
| Verdicts Keep Their Findings | `rune review close --verdict accept\|changes\|reject`. `rune review export` reads `[ISSUE]`, `[NOTE]`, `[SUGGESTION]`, and `[PRAISE]` markers with `.rune-comments.yaml` | none | the refusal names the file and line | nonzero on a refused `accept` |
