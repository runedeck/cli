# Proofs share one format: design

## Approach

The proof runner wires five outside formats and tools together and keeps only the glue:

```text
README.md (Setup, scenes, Teardown; a live scene adds "### On screen")
   │ rune proof run <change>: checkout of head, allowlisted environment, one chain
   ├─ command blocks (scrut library) ──▶ scenes/<slug>/transcript.md
   │                                     proof.cast (paced render of the run)
   ├─ model prompts (rune run)       ──▶ scenes/<slug>/transcript.md
   └─ screen blocks (tui-test, pty)  ──▶ scenes/<slug>/screen.md + screen.cast
   provenance (SLSA v1) + match (in-toto test-result) ───────────┐
   rune proof judge ─▶ judgment ── rune proof approve ─▶ approval ┤
                                                                 ▼
                                                     proof.intoto.jsonl
   rune proof check [--rerun] ─▶ text │ CTRF JSON (panes, CI) │ dashboard /proofs
```

The alternative was to grow the console fence grammar with shared state, regex lines, and screen steps. It lost because the parser and matcher would grow to cover what scrut and tui-test already do, and the grammar would stay one that only rune reads. The ADR records the full comparison. `migration.md` holds the plan for every existing proof and the converter's rules.

## Format

The body uses scrut's Markdown format [SCRUT-MD]. A ` ```scrut ` block holds one shell expression: `$ ` starts it and `> ` continues it. Every other line is expected output. `[N]` states a non-zero exit code. A line can end in `(glob)`, `(regex)`, `(escaped)`, `(no-eol)`, or a quantifier such as `(glob*)`. rune adds these rules:

| Rule | Reason |
|---|---|
| `## Setup` runs first. Its `### On screen` blocks run only when screen blocks run. | One place for the off-camera fixtures the bash proofs build today. |
| Every other H2 is a scenario key, `<capability>#<requirement-slug>/<scenario-slug>`, then `## Teardown` last. | The key the graph exporter mints. No H2 is ignored. |
| The first paragraph under a key is the claim. A paragraph that starts with `> Note:` is context. | The judge reads the claim and the context apart, as `then` and `note` today. |
| A live scene holds its screen blocks under `### On screen`. | The command blocks of the same scene still run in CI. |
| Command blocks, and separately screen blocks, run as one chain in document order, with `fail_fast: true`. A model prompt starts from Setup alone. | Scenes tell one story. This is scrut's own model. Prompts are independent. |
| `## Teardown` runs after the chain stops for any reason. | The chain stops at the first failure. |
| Each scene gets `scenes/<slug>/`. The slug is the scenario slug, prefixed by the requirement slug on a collision. | One folder of evidence per scene. |

Frontmatter: `type: proof`, `change`, `scenes[] {scenario, kind, model, needs}`, `inputs`, `tools`, `env`, and scrut's `defaults`. `kind` is `check`, `new`, `run`, `instruction`, `live`, or `unproven`. `needs` holds `host` or `harness:<profile>`. `inputs` are hashed repository paths outside `fixtures/`. `tools` are programs whose `--version` goes into the record. `env` names inherited variables, and rune records their names only. `head`, `recorded`, and `transcript` leave the frontmatter, because the record holds them.

Screen blocks call tui-test [TUITEST-CLI]: `run --cols C --rows R --cwd DIR PROGRAM`, `submit`, `type`, `key press`, `expect text TEXT [--not] [--regex] --timeout MS`, `wait idle`, `text`, `close`. rune wraps each screen part in `record start <scene>/screen.cast` and `record stop`, and sets `TUI_TEST_SESSION` to `proof-<change>`. A tui-test block ends with `* (glob*)`, because its success output is not documented. The proof runner pins the key screen lines with glob lines, and the judge reads the whole screen.

## Files

```text
docs/proofs/<change>/
  README.md                    source, written by the author, first by scaffold
  fixtures/                    inputs, including copies from other repositories
  scenes/<slug>/transcript.md  command or model part, written by scrut's generator
  scenes/<slug>/screen.md      screen part: every `tui-test text` screen
  scenes/<slug>/screen.cast    screen part recording, asciicast
  scenes/<slug>/judge.md       the judge's raw reply
  scenes/<slug>/files/         artifacts a block wrote to $RUNE_SCENE_DIR
  proof.cast                   paced recording of the command scenes, rendered from the run
  proof.intoto.jsonl           the record, one in-toto statement per line
```

`proof.txt`, `proof.gif`, `live.yaml`, `live/`, and the `.ansi` captures retire. `proof.cast` stays in the layout of the change proof-cast-reads-scene-by-scene: a rule per capability, a boxed `N/T · title` and a kind line per scene, each command typed behind a prompt, a tick or cross per step, and a tally, paced by `--pause`. Its writer (`src/cli/proof/cast.rs`) reads scrut's results instead of the console runner's. The `$#` comment line becomes a one-line paragraph directly above a block. The recording is a render: provenance names it under `runDetails.byproducts` with its digest, and no status depends on it. GIFs render on demand with `agg` from `proof.cast` or a `screen.cast` into `$XDG_CACHE_HOME/rune/proofs/gif/<sha256>.gif`.

## Record and status

| Statement | Predicate type | Written by | Subject |
|---|---|---|---|
| provenance | SLSA provenance v1 [SLSA] | `run` | the part's evidence files and `files/*` |
| match | in-toto test-result v0.1 [INTOTO-TR], `result` PASSED or FAILED | `run` | the same files |
| judgment | `https://github.com/runedeck/cli/review/v1`, `role: judgment` | `judge` | the same files |
| approval | the same, `role: approval` | `approve` | the same files |

Provenance `resolvedDependencies` holds head (`gitCommit` plus the jj change id as an annotation), the digest of each section the chain ran through (`README.md#setup`, `#<earlier-slug>`, `#<slug>/on-screen`), every fixture, and every input. A section digest covers only its fence lines and block bodies, so a prose edit stales nothing. The claim has its own digest, `#<slug>/claim`, which only a judgment binds. The review predicate takes EARL's outcome words [EARL]: `passed`, `failed`, `cantTell`. Its fields are `scenario`, `role`, `by`, `outcome`, `reason`, `quote`, `ux`, `inputs`, `overrules`, and `date`.

| Status | When | CTRF | Glyph |
|---|---|---|---|
| unproven | kind `unproven`, or no evidence | pending | `·` |
| stale | a subject or dependency digest differs from the file now | failed | `!` |
| failed | a current match failed, an approval rejects, or a failed judgment is not overruled | failed | `✗` |
| pending | a judgment or approval is missing, or only `cantTell`, or a live scene lacks its screen part | other | `?` |
| proven | none of the above | passed | `✓` |

## Commands

| Requirement | Command and flags | Exit status |
|---|---|---|
| Skeleton Is Written First | `rune proof scaffold <change>` | 2 on an existing README |
| Scenes Run In One Chain, A Run Uses A Checkout Of Head, Each Scene Writes A Transcript, Command Scenes Get A Paced Recording | `rune proof run <change> [--scene KEY] [--pause SECONDS]` | 1 on a failed scene |
| Scenes Declare What They Need | `rune proof run --host`, `rune run <profile> --check` | 0, the scene skipped |
| Console Proofs Convert Once | `rune proof convert <change>` | 2 on an unconvertible line |
| Screen Steps Drive A Terminal, Screen And Model Parts Run On Request | `rune proof run <change> --screen`, `--model`, then `tui-test daemon start` and `record start` and `record stop` around the screen steps | 1 on a failed part |
| A Judge Reads Only The Evidence | `rune proof judge <change> --profile P [--scene KEY]`, `.rune` key `proof.models`, `rune run P --clean --json --mode answer-only` | 1 unless every judgment passed |
| Only The Owner Approves | `rune proof approve <change> [--scene KEY]` | 2 without a terminal |
| One View Serves Every Reader, The Check Fails Unless Proven | `rune proof check [<change> \| --all] [--json] [-v]` | 1 on stale, failed, or pending |
| CI Runs The Command Blocks Again | `rune proof check --all --rerun` | 1 on a failed block |
| The Dashboard Plays Recordings | `rune dashboard`, route `/proofs` | none |
| Images Render On Demand | `rune proof gif <change> <scene>` | 1 when a committed image is reported |

Every other requirement is a property of these commands. Command and diagnostic names are proposed here. The implementer may rename them and then updates this table.

## Coverage table

| Capability | Requirements | Check |
|---|---|---|
| every-scenario-has-proof | all | unit tests of the adapter, `tests/proof_run.rs`, this change's own proof |
| proof-record-binds-evidence | all | record and status unit tests, a fixture bundle, `rune spec validate` |
| live-scenes-need-approval | Screen Steps Drive A Terminal, Only The Owner Approves | recorded on the host only, so a missed requirement in CI. The live proofs of mods prove them |
| live-scenes-need-approval | the others | unit tests with a stub judge |
| proofs-render-one-view | all | a CTRF fixture shared by the rune and the pane tests, schema validation |

## Structure

- `src/cli/proof/`: the scrut adapter (section split with pulldown-cmark, cross-checked against scrut's parser), the chain, the checkout, the environment, `needs`, the tui-test probe, the judge, the approval loop, and the converter.
- `src/manifest/`, `src/proof.rs`: one generic serde set of Statement, ResourceDescriptor, and SLSA structs, which `src/services/sidecar.rs` and `src/manifest/provenance.rs` also use. Bundle read and write, section digests, status.
- `src/view.rs`: `ProofView`, its `Display`, and the CTRF structs. `src/cli/dashboard/`: the `/proofs` route with asciinema-player pinned.
- `src/cli/graph/lifecycle.rs`: proves edges from the check status.
- `src/cli/proof/cast.rs` stays and reads scrut's results.
- Retired: `src/cli/proof/fence.rs` (539 lines on main of 24 October), `matcher.rs` (152), most of `runner.rs` (490), `docs/proofs/driver.sh`, `cast.py`, every `record.sh`.

## Spike findings

scrut 0.4.3 was built from crates.io on 22 October 2026 with `cargo install --locked scrut --version 0.4.3` (binary sha256 `3f9205f7188b09b189393c4f0e822a3d4fd6509d4ab8558b0f01d87d0604665c`). Its `--version` prints a build timestamp, a bug that 0.5.0 fixes. A proof-shaped document ran under `scrut test` and showed:

- rune's frontmatter keys are ignored, and `defaults.output_stream: combined` applies to every block.
- `cd` and exported variables persist between blocks. A heredoc works in `>` continuation lines. Combined output keeps the order of stdout and stderr. `* (glob*)` matches zero or more lines. `[1]` matches an exit code.
- The test title is the nearest heading, so a block's title is its scenario key.
- The chain stops at the first failure only with `defaults.fail_fast: true`, and later blocks are reported as skipped. The proof runner sets it.
- The binary sets `TESTDIR`, `TMPDIR`, `LANG=C`, `LC_ALL=C`, `TZ=GMT`, and `COLUMNS=80` in `src/bin/utils/environment.rs`. The library sets none of them, so the adapter sets the same values, including `COLUMNS=80`, so `scrut test` on a README gives the same output.
- `scrut update` rewrites expected output and keeps headings and prose, so `MarkdownUpdateGenerator` writes transcripts.
- The library exposes `MarkdownParser`, `StatefulExecutor`, `MarkdownTestCaseGenerator`, and `MarkdownUpdateGenerator`. No config struct denies unknown fields.

## Risks

- scrut is young: 84 stars, and 0.5 breaks the library interface while crates.io holds 0.4.3. The spike in task group 1 checks every assumption. If it fails, option 1 of the ADR keeps this record and view.
- tui-test's recording format version, its no-break space matching, and the environment its daemon gives to `claude` are unverified. The spike checks them on one live scene of mods.
- A dropped line in the record can turn failed into proven. Signed commits and the merge seal guard the file.
- A pending scene fails CI, so a change with live scenes cannot merge before the owner approves. That is the owner's decision of 22 October 2026.

[SCRUT-MD]: https://facebookincubator.github.io/scrut/docs/reference/formats/markdown-format/
[TUITEST-CLI]: https://raw.githubusercontent.com/microsoft/tui-test/main/references/cli.md
[SLSA]: https://slsa.dev/spec/v1.0/provenance
[INTOTO-TR]: https://raw.githubusercontent.com/in-toto/attestation/main/spec/predicates/test-result.md
[EARL]: https://www.w3.org/TR/EARL10-Schema/
