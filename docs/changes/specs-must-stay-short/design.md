# Spec House Rules Design

## Approach

The house rules are one lint layered over the OpenSpec compatibility parser, so the parser keeps accepting upstream artifacts and the rules can change without a parser fork. This beat a stricter parser, which would reject the v1.6.0 oracle fixtures, and it beat a separate script, which runs only where someone invokes it. [CLI-0040](../../decisions/CLI-0040%20Specification%20House%20Rules.md) records the decision.

## Structure

- `rune-docs/src/spec/lint.rs`: the keyword, length, and term rules, shared by validate and doctor.
- `rune-docs/templates/spec/delta-spec.md`: the delta template uses MUST.
- `.pre-commit-config.yaml` and `.github/workflows/build.yaml`: the `rune-spec-doctor` hook and `REQUIRE_RUNE`.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids | Exit status |
|---|---|---|---|---|
| House rules are a separate lint | `rune spec validate`, `rune spec doctor` | `rune-spec-doctor` | `spec-shall-keyword`, `delta-shall-keyword` | 1 on an error |
| House rules run in the commit-stage checks | `rune spec doctor` for every change under `docs/specs/` or `docs/changes/` | `rune-spec-doctor`, with `REQUIRE_RUNE` set by the Build quality job. A fresh clone before `make install` skips | none | the hook fails with the doctor's status when the binary is required |
| Capabilities stay short | the same | | `spec-too-long` (error), `delta-too-long` (warning) | 1, and 0 for the warning |
| Defined terms have a glossary entry | the same. Pages under `docs/specs/glossary/`, written by `rune spec glossary --write`, migrated by `rune spec fix --links` | `rune-spec-doctor` | `spec-term-undefined`, `delta-term-undefined`, and the other term ids in the `every-term-cites-definition` design | 1 |

## Risks

- The two term deltas, this change's and `every-term-cites-definition`, describe one rule twice until both archive. The later one supersedes.
- A consumer that lints with this release converts SHALL to MUST and splits any capability over the limit.
