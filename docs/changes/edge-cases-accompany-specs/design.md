# Design

## Approach

The specification stays a valid OpenSpec file: requirements with keywords and one scenario each, so the compatibility parser and the OpenSpec checker read it unchanged. Everything this change adds sits beside it. The companion `edge-cases.md` opens with `## Edge cases`, repeats the specification's `### Requirement:` headings, and holds `#### Edge case: <Title>` blocks where the specification holds scenarios. A block has `- **WHEN**`, `- **THEN**`, optional `- **AND**` steps, and an optional `- **DECISION**:` line. The companion carries no `## ADDED Requirements` or other delta operation header. OpenSpec 1.14.1 reports any Markdown file beside `spec.md` that carries one as a misplaced delta. It reads a file without one as a note. An edge case takes the operation its requirement has in the specification.

The proof runner keys an edge case like a scenario, `<capability>#<requirement>/<edge-case>`, so scenes, casts, and the graph need no second shape. The generator writes the review view to standard output, or to a path the caller names outside the tree. Its first line is a marker, and the validator reports any tracked Markdown file under the specification root that starts with it.

Three alternatives fell in the review. Keeping every scenario in the specification: too much text for one owner. A maintained review file: a second source of truth that drifts, which the owner called a derived artifact and more stuff to get into conflict. Moving all scenarios out: the specification would be statements nothing can fail.

## Coverage table

| Capability | Requirement | Check or missed requirement |
|---|---|---|
| edge-cases-accompany-specs | Requirements Use Three Keyword Levels | the legacy-keyword finding for `SHALL`, and a missed requirement for a body with no keyword |
| edge-cases-accompany-specs | Each Requirement Keeps One Primary Scenario | the second-scenario finding |
| edge-cases-accompany-specs | Edge Cases Live Beside The Specification | the edge-case shape finding, and review for a missing decision line |
| edge-cases-accompany-specs | Proofs Cover Every Edge Case | the proof check's unproven report |
| edge-cases-accompany-specs | The Review View Is Generated On Demand | the committed-view finding, and the generator's tests for content and order |

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids | Exit status |
|---|---|---|---|---|
| Requirements Use Three Keyword Levels | `rune spec validate [NAME]` and `rune spec doctor` | `rune-spec-doctor` | `spec-shall-keyword` and `delta-shall-keyword` (unchanged). The parser's `must contain SHALL or MUST` refusal widens to the three levels | nonzero on `SHALL` |
| Each Requirement Keeps One Primary Scenario | `rune spec validate [NAME]` and `rune spec doctor`. Configuration `spec.primary_scenario: warn` (default) or `error` | `rune-spec-doctor` | `spec-second-scenario`, `delta-second-scenario` | 0 while a warning, nonzero once an error |
| Edge Cases Live Beside The Specification | `rune spec validate [NAME]` and `rune spec doctor`. `rune spec archive <id>` carries the companion | `rune-spec-doctor` | `spec-edge-case-invalid`, `delta-edge-case-invalid` | nonzero |
| Proofs Cover Every Edge Case | `rune proof scaffold <change>`, `rune proof run <change>` | none | the unproven verdict line, keyed `<capability>#<requirement>/<edge-case>` | nonzero on an unproven scene |
| The Review View Is Generated On Demand | `rune spec view <change-id>`, `--out <path>` outside the tree | none | `spec-view-committed` | nonzero on a committed view |

The command name `rune spec view`, the configuration key, and the diagnostic ids are proposed here. The implementer may pick other names, and then updates this table.

## Structure

- `rune-docs/src/spec/parse.rs`: the keyword rule accepts MUST, SHOULD, and MAY. The companion parser lives here too, used by the lint, the proof runner, the archiver, and the generator.
- `rune-docs/src/spec/lint.rs`: `lint_scenarios` (second scenario) and `lint_edge_cases` (companion shape), shared by validate and doctor.
- `rune-docs/src/spec/apply.rs`: the companion moves with the delta's requirements into `docs/specs/<capability>/edge-cases.md`.
- `src/cli/proof/mod.rs`: `scaffold` and `run` enumerate edge cases after scenarios.
- `src/cli/spec/`: the generator behind `rune spec view`.
- `rune-docs/templates/spec/delta-spec.md` and `templates/spec/delta-spec.md`: one scenario and a companion stub.

## Risks

- A SHOULD or MAY requirement with no MUST fails the OpenSpec checker, which requires `SHALL` or `MUST` in every requirement body. Today it fails the compatibility parser for the same reason. The parser widens to the three levels in this change. The OpenSpec checker does not, so the two checks disagree until OpenSpec accepts the levels or leaves the commit path. The owner decides which check the commit path trusts.
- An unmarked edge case changes meaning and never reaches the owner. A reviewing model can read the companion for unmarked cases that change meaning, as proposed in the review and not decided. Until then it is review work.
- Two sources drift when an author edits a requirement and forgets its companion. The shape finding reports a block under a requirement the specification no longer has.
- The migration leaves scenarios in place and the warning becomes noise. A task raises the level once the tree is clean.
