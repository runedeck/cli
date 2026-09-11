# Validation record

This file separates completed checks for signed revision `7412b6acf037420129c8ac45759d4c75af0e96af` from later results.
Follow-up assertions have focused results below, but their publication and revised CI checks remain pending.
These results do not establish permission to deploy or merge.

## Current state

The source snapshot, reserved build provenance, migration controls, and native evidence adapter are implemented.
Their recorded local checks and independent implementation review preceded the expanded source-layer contract.
The generic, harness, model, and rendered lint gates are implemented.
The final library, focused integration, and Clippy checks pass after the consistency corrections.
The content CI block passes all 32 tests without skips.
The provider, configuration, and deployment corrections pass their focused tests.
The expanded Deck scan checks 110 text paths with zero findings.
The full Cargo log records 1519 passed tests across 34 result groups, with no failures or ignored tests.
The immutable publication gate passed for `7412b6a`, including its Cargo test gate, and published that bookmark.
Both Linux and macOS Skill readiness jobs passed at the same head.
N01/N02, native duplicate-through-alias rejection, external verifier isolation, ADR acceptance, and integration remain open.
The strengthened T04/T05/T09–T12 assertions passed later focused runs.
The two new static alias controls also passed their separate focused run.
These follow-up changes still require their own immutable publication gate and revised CI run.

## Publication and platform evidence

[Skill readiness run 34571451799](https://github.com/runedeck/cli/actions/runs/34571451799) completed successfully on
2026-09-11 at `7412b6acf037420129c8ac45759d4c75af0e96af`.
Both `contract (ubuntu-latest)` and `contract (macos-latest)` passed their required steps.
Each job executed 40 Rust integration tests across five targets and 35 Python controls without failures or skips.
The Python controls comprise 21 artifact-runner cases and 14 native-adapter cases.
Those controls include zero-test, required-skip, missing-executable, and stale-pin rejection.
They do not prove isolation from a worker who can modify the verifier or its expected hashes.

The macOS job passed `case_aliases_and_symlinks_are_accounted_for`.
That test includes a case-insensitive temporary-filesystem assertion.
The newly added standalone workflow probe requires a separate run at its next published head.
No result from this run establishes native discovery through an alias.

The retained full-suite log is `runedeck-final-full-tests.log`, with SHA-256:
`7bb0eb31404e055041cd1f498be5e7654850f926e4658eb0dbe1947380c004de`.
Its 1519-test total comes from all 34 result summaries, including the empty result group.
The retained immutable publication log is `runedeck-final-cli-push.log`, with SHA-256:
`06ca48608afff69850e66be7d71d54840250ee42c78d55635724c85dda88b6fd`.
It identifies `7412b6a`, the isolated checkout, passing applicable gates, and the published bookmark.
Shellcheck and TypeScript report no applicable files; required test suites are not skipped.
The coordinator retains both logs outside tracked source.

## Follow-up assertion checks

After `7412b6a`, the existing fixtures gained exact codes and affected paths for T04/T05/T09–T12.
Migration refusal controls cover both edited entrypoints and edited companions, with and without `--force`.
The retained-copy test asserts two duplicate paths after `--no-prune`, then one clean static candidate after migration.
Its native status remains unverified throughout.
These changes add and strengthen tests without changing production code.
Independent review completed, and the focused migration and identity runs passed.

| Focused result | Observed count | Retained log SHA-256 |
| --- | --- | --- |
| `runedeck-followup-migration-tests.log` | 12 passed, zero failed or ignored; 716 unrelated cases filtered out | `558cc17a0921e6d884cbfb46c6781ecd46760468946978dae488437cf6f01a81` |
| `runedeck-followup-identity-tests.log` | 18 passed, zero failed, ignored, or filtered out | `6b1f9b12a784e6a948cf455df8751f2c3e1bb26f2b55cab0f6a68de7c3a37040` |
| `runedeck-followup-alias-tests.log` | Two passed, zero failed, ignored, or filtered out | `11e1618d2089ac8666ae4bdcd9fe7dba0e791db4d3bcb180b22e8f43b72555b4` |

These logs describe the working-tree follow-up, not the earlier signed head.
The tested deployment fixture file has SHA-256
`b18874806542bd51bcfa74886127319acc87e5ebb57f75cc7f02ead5c68f924e`.
The tested identity integration file has SHA-256
`c38ed25b93e324b6b04b2b7b2d22a5f5d6c1774dd5ba7e9ca63f0dc02b95648d`.

The new `codex_skill_aliases` target passed both additional controls.
Its fixture file has SHA-256 `88ce1c6b44c2bb1413aa1214566dfe10c8bfc421da531f112e33136fe1e0a273`.
They require exact duplicate identities and logical paths for sibling directory aliases and distinct physical bundles.
This conservative static behavior follows the existing design; it does not claim that the harness loads both alias paths.
The current workflow includes this sixth target and a standalone macOS filesystem probe.
Neither change inherits the five-target `7412b6a` CI result.
The next immutable publication and revised workflow results remain pending.
Current `make validate` passes all applicable checks.
Its log contains 11 passed hook rows, one inapplicable TypeScript skip, and a clean secret scan.
The retained `runedeck-followup-validate.log` has SHA-256
`ac0ef8cfbcce281f12b6df6fb6b8f136c7d2941221a1a79cc672deafb76df715`.

## Focused results

These earlier results describe implementation checkpoints. The completed head-bound checks appear above.

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
These focused controls do not establish trusted external-runner isolation.

The serialized full Cargo run covered 1462 tests: 1460 passed and two outdated default-path assertions failed.
Those assertions now check `.agents` placement and both provider manifest paths.
The corrected provider and prune targets then passed all 22 tests.
That earlier run included 450 library tests, 728 binary tests, and the macOS case-insensitive filesystem probe.
All 17 identity tests and three metadata tests passed in that run.

`make validate`, formatting, and all-target Clippy pass with warnings denied.
The new workflow passes actionlint and the offline workflow audit.
SafetyFirst's earlier 12-test suite passed before the later layer and probe controls.
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
`make validate` passed the applicable gates on the expanded candidate, including formatting, Clippy, Rune validation,
and Gitleaks.
SafetyFirst passes all 13 tests, including the new layer gate, in 34.428 seconds.
These focused results preceded the completed `7412b6a` publication gate and do not replace it.
The architecture commit-stage repository checks passed after the Python runner was added.
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
All-target, all-feature Clippy and the applicable repository gates passed after the corrections.
The native evidence adapter passes all 14 Python controls.
The completed immutable publication check for these corrections is recorded above.
The earlier full-suite attempt passed 480 library and 728 binary tests but failed one of 25 deployment tests.
That failure exposed the CLI `agents` alias selecting Codex through the shared `.agents` target.
The focused repeat now passes the strengthened deployment assertion.
The later serialized full suite passed all 1519 tests for the signed `7412b6a` candidate.

The canonical ADR notes retain their accepted status and original design history.
They now identify current skill user-replacement and provenance-path behavior.
Repository agent guidance uses nested provider/model qualifiers and explains the complete user-entrypoint exception.
These corrections are included in signed revision `7412b6a` and its completed publication check.
Further changes require a fresh signature and publication gate.

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

### Directory-alias catalog observation

A separate catalog-only observation ran on 2026-09-11 at 08:10 UTC with Codex Desktop 0.153.4 on macOS 26.6.2, arm64.
The disposable fixture included `.agents/skills/csi-alias-link` pointing to `csi-alias-original` in the same directory.
The catalog contained one enabled `csi-alias-canary` at `.agents/skills/csi-alias-original/SKILL.md`.
The alias emitted no second entry. This does not distinguish an ignored alias from internal deduplication.
The catalog reported no errors, and the fixture inventory remained unchanged.
The client sent only `initialize`, `initialized`, and `skills/list`; it did not start a model turn.
The raw record also contains a server status notification.

The adapter SHA-256 matches the adapter at signed revision `7412b6a`:
`ef2ae6ffe9c924cb5009aef0d5ddadad66ad6d14a059cb219b9684609713983b`.
The harness binary SHA-256 is `87a08119b8effa519f0ecb552dc98043f58a8200bf2ec5da60f76890c33e9c3a`.
The raw JSONL SHA-256 is `3635dd19fe3ba15922fdc91525f5e78276c62ca30b3102f784aad64faa09e3e5`.
The canonical fixture-inventory SHA-256 is
`99178911d428fd98a17a480a48febb8431a5b6cc17097d232bd9517696e73b61`.
This inventory digest does not identify the enclosing `fixture.json` file's bytes.

No static doctor check ran for this fixture, and no duplicate native entry was emitted through its alias.
The observation records catalog behavior only. Duplicate rejection, companion access, and overall acceptance remain unverified.

### Remaining native acceptance

N01 and N02 remain incomplete.
Earlier normal native startup failed with `failed to initialize sqlite state runtime` and `Operation not permitted`.
The supported app-server proxy could not find its `app-server-control.sock` socket at that checkpoint.
The later catalog-only observations below cover every selected path but retain same-name enabled user copies.
Neither context has a representative canary invocation or companion-read record.
The alias observation above does not establish T18's duplicate-through-alias rejection case.

### Selected Workshop and Deck catalogs

Separate catalog-only observations ran on 2026-09-11 against disposable deployments and a preserved Rune binary.
Its SHA-256 is `10ff50aad372baa79a18fb1381ae4b442b8d42352847b0a32f4d59ac5241e1c7`.
The observed harness and adapter match the versions identified in the alias observation above.
The client sent only `initialize`, `initialized`, and `skills/list` in both contexts. No model turn ran.

The Workshop recipe used its seven include entries and produced four selected skill bundles:
BuildSkill, RTK, SimplifiedTechnicalEnglish, and VersionControl.
All four appeared enabled at their expected workspace paths, with no missing or extra workspace entries.
The full catalog contained 72 entries, including 68 outside the workspace.
Enabled user copies of BuildSkill, SimplifiedTechnicalEnglish, and VersionControl remained visible at separate paths.
These three same-name pairs are an observed uniqueness blocker, not a missing-catalog result.

The first Deck recipe used an unsupported wildcard selection, `include: ["*"]`, and failed before native observation.
It exited two with `error.config` because the requested rune `*` did not exist.
The coordinator preserved that failed fixture and authorized a separate corrected recipe.
This was a fixture-selection error, not a source-skill defect.

The corrected recipe explicitly selected all 14 canonical skill IDs.
The authored Claude-only target on `core/skills/ste` excluded it from Codex, leaving 13 expected skills.
All 13 appeared enabled at their expected workspace paths, with no missing or extra workspace entries.
The full catalog contained 81 entries, including the same 68 external entries.
Nine names also had enabled user copies: AdoptArtifact, BenchArtifact, BuildAvatar, BuildSkill, IntakeIdea, SafetyFirst,
SimplifiedTechnicalEnglish, VersionControl, and `rune`.
These observed same-name copies remain blockers to a unique discovery verdict.

Both successful installations reported no warnings or errors. Both catalogs reported no errors.
The retained summaries bind the selected bundle digests and paths to these source snapshots and raw observations:

| Context | Source snapshot digest | Raw JSONL SHA-256 |
| --- | --- | --- |
| Workshop | `261203b23aeeb56ebb2306c474c3b379ac53b25f23d0c511467a456cc792bff7` | `5c0c5a11e5aae86058bc73be7bc4d499ef01365a3651c383a5da80c8ad08d1fe` |
| Deck, explicit selection | `b8afb4e5aed92c62c03626bcd57248e3e75e6e3bb33b392bed81d692149a8f85` | `97c143723070fe02e06520239c624d906deffb68b0d9e2e1eee7b42c7df1dd2f` |

These records establish selected-path catalog visibility for the inspected snapshots.
They do not establish invocation, companion access, uniqueness across the full scope, or overall readiness.

## Evidence handling

Keep raw native transcripts outside tracked source files because they contain local paths and session identifiers.
Record later tool versions, counts, failures, and artifact digests against each new tested revision.
Do not infer overall readiness from static success or from the N03 negative control.
