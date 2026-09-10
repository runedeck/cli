# Validation record

This file records the integrated candidate. It does not establish permission to deploy or merge.

## Current state

The source snapshot, reserved build provenance, migration controls, and native evidence adapter are implemented.
Their recorded local checks and independent implementation review preceded the expanded source-layer contract.
The generic, harness, model, and rendered lint gates are implemented.
The final library, focused integration, and Clippy checks pass after the consistency corrections.
The content CI block passes all 32 tests without skips.
The provider, configuration, and deployment corrections pass their focused tests.
The expanded Deck scan checks 110 text paths with zero findings.
The final full Cargo run passes all 1519 tests across 34 result groups, with no failures or ignored tests.
The immutable publication check remains pending.
The publication gate repeats the full suite against the immutable outgoing commit.
The PR records that gate's result and the exact published revision.

## Focused results

| Focused check | Observed result |
| --- | --- |
| `rtk proxy cargo test --offline --lib source_snapshot` | 8 passed |
| `rtk proxy cargo test --offline --bin rune cli::assemble::snapshot::tests` | 5 passed |
| `rtk proxy cargo test --locked --test codex_skill_metadata` | 3 passed |
| `rtk proxy python3 -m unittest discover -s scripts -p test_check_codex_skill_discovery.py` | 14 passed |
| `rtk proxy cargo test --locked --lib skill_readiness::native` | 8 passed |
| `rtk proxy cargo test --locked --bin rune skill_symlink` | 9 passed |
| `rtk proxy cargo test --bin rune cli::deploy::tests:: --all-features` | 54 passed |
| `rtk proxy cargo test --all-features --test codex_skill_portability --test skill_layers` | 15 passed: two rendered and 13 source-layer tests |

Each command exited zero. These groups overlap the broader suites; do not add their counts to full-suite totals.
The metadata target executes a harmless Python sibling import with a five-second limit.
It also checks deployed provider exclusion, absent optional metadata, and whole nested-map replacement.
The native parser controls include failed startup and malformed, empty, stale, and incomplete evidence.
The 54-test deployment run includes public migration retry, failed-copy recovery, and two distinct-root coexistence
cases.
They do not establish trusted external-runner integrity or prove that an external runner executed a nonzero test set.

The serialized full Cargo run covered 1462 tests: 1460 passed and two outdated default-path assertions failed.
Those assertions now check `.agents` placement and both provider manifest paths.
The corrected provider and prune targets pass all 22 tests. No implementation change followed that full run.
The full run includes 450 library tests, 728 binary tests, and the macOS case-insensitive filesystem probe.
All 17 identity tests and three metadata tests passed in that run.

`make validate`, formatting, and all-target Clippy pass with warnings denied.
The new workflow passes actionlint and the offline workflow audit.
SafetyFirst's 12 tests pass against the final candidate binary.
Source and executable bytes must remain stable during checks.
These earlier results precede the layer-gate results below.

## Layer-gate results

The source gate checks generic, user, harness, and exact-model source layers and their resolved instructions.
Rendered doctor inspection reuses the Codex policy and reports `CSI007_HARNESS_LEAKAGE`.
The expanded library suite passes all 480 tests, including 17 source-layer and 13 portability controls.
That library result came from `rtk proxy cargo test --all-features --lib --test codex_skill_portability --test
skill_layers`.
The same command later failed one rendered integration case because its module fixture omitted required metadata.
The fixture now supplies that metadata and source provenance.
The focused integration repeat passes all 15 tests: 13 source-layer tests and two rendered portability tests.
All-target, all-feature Clippy passes with warnings denied after three local style fixes.
`make validate` passes all 12 hooks on the expanded candidate, including formatting, Clippy, Rune validation, and
Gitleaks.
SafetyFirst passes all 13 tests, including the new layer gate, in 34.428 seconds.
The final immutable publication gate remains pending. Focused results do not substitute for that gate.
The final architecture commit-stage checks pass all 12 applicable hooks, including the new Python runner.
Static model checks establish declared constraints only. Native and model-quality evidence remain separate.

### Existing deck baseline

The isolated deck baseline used `rune validate --skill-layers --json --source <deck-clone>` and exited one.
It inspected 106 text paths and reported 390 `CSI007_HARNESS_LEAKAGE` findings.
Repeated effective provider/model checks contribute to that total. It does not represent 390 unique authored defects.
The authored subset contains 30 findings on nine paths across five skills:
AdoptArtifact, BenchArtifact, BuildAvatar, BuildSkill, and SimplifiedTechnicalEnglish.
The findings include harness tool names, Claude call syntax, injection syntax, and Claude runtime variables in generic
text.
Quoted examples remain findings under the accepted literal policy.

The coordinator retains the machine-readable baseline outside tracked source files.
These findings remain failures without suppression or automatic rewrites.
The user expanded the content scope to resolve these five skills, SafetyFirst, and affected skill-authoring guidance.
The fresh baseline checks 110 text paths and reports zero findings after those corrections. Unrelated work from the
active Claude run remains outside this scope.

## Consistency corrections

Source target checks now reuse resolved provider configuration and preserve all content-selector matches.
Unavailable source paths return the versioned `SL000_INVALID_INPUT` report.
The CLI provider resolver separately rejects ambiguous aliases or target-directory matches.
The current full library suite passes all 488 cases.
Provider unit tests pass all 43 cases, including five new selection controls.
Focused integration passes all 43 cases: 25 deployment, five configuration, and 13 source-layer tests.
All-target, all-feature Clippy and all 12 repository hooks pass after the corrections.
The native evidence adapter passes all 14 Python controls.
The full immutable publication check remains pending.
The earlier full-suite attempt passed 480 library and 728 binary tests but failed one of 25 deployment tests.
That failure exposed the CLI `agents` alias selecting Codex through the shared `.agents` target.
The focused repeat now passes the strengthened deployment assertion.
The full serialized suite will run against the final signed revision.

The canonical ADR notes retain their accepted status and original design history.
They now identify current skill user-replacement and provenance-path behavior.
Repository agent guidance uses nested provider/model qualifiers and explains the complete user-entrypoint exception.
These corrections changed the previously signed CLI candidate. A fresh signature and publication gate are required.

## Documentation checks

The expanded eight-requirement contract passes OpenSpec strict validation.
The draft ADR and both corrected canonical ADRs pass the existing mdschema.
The scoped offline link check reports 39 valid references and no errors; seven external references are excluded.
Rumdl passes all eleven touched Markdown files, including repository guidance and both canonical ADRs.
No additional mdschema is configured for the other documents in this change directory.

## Native observations

The first native catalog probe observed the disposable canary once, enabled, under `.agents/skills`.
It used Codex Desktop 0.153.4 on macOS and returned no discovery errors.
A catalog observation alone does not prove companion access or overall readiness.

N03 observed two enabled entries named `csi-duplicate-canary` with no catalog errors:

- `.agents/skills/canary-a/SKILL.md`
- `.agents/skills/canary-b/SKILL.md`

The observation used Codex Desktop 0.153.4 on macOS 26.6.2, arm64.
Doctor exited one and reported `CSI001_DUPLICATE_NAME` plus incomplete source identity.
Its record has `static_valid: false`, `native_discovery: unverified`, and `accepted: false`.
This proves that native duplicate entries remain visible. It does not establish a successful native canary invocation.

The retained N03 raw JSONL digest is:
`d5e43da784155522aa61e777325d73f7df6ba5f8fefa05151f44dc9fd9d48ef1`.
The coordinator retains `raw.jsonl`, `catalog.json`, and `static-report.json` outside tracked source files.

N01 and N02 remain incomplete.
Normal native startup failed with `failed to initialize sqlite state runtime` and `Operation not permitted`.
The supported app-server proxy could not find its `app-server-control.sock` socket.
Neither context has a completed representative canary invocation or companion-read record.
The native duplicate-through-alias case also remains unobserved.

## Evidence handling

Keep raw native transcripts outside tracked source files because they contain local paths and session identifiers.
Record final tool versions, counts, failures, and artifact digests after the coordinator completes the checks.
Do not infer overall readiness from static success or from the N03 negative control.
