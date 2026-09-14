---
adr: "docs/decisions/CLI-0040 Specification House Rules.md"
status: proposed
---

# Spec House Rules

## Why

The skeleton's review-ceremony specification grew past three hundred lines, mixed SHALL with MUST, and defined its own terms in passing with nowhere to look them up. The parser accepts both keywords because OpenSpec artifacts use either, so the house style needs its own check. [CLI-0040](../../decisions/CLI-0040%20Specification%20House%20Rules.md) records the decision to keep compatibility in the parser and put the house rules in a separate lint.

## What Changes

- `rune spec validate` and `rune spec doctor` report a prose line that uses SHALL, a canonical specification over 150 lines (a delta over that length warns), and an italic defined term with no entry in `docs/specs/glossary.md`.
- The OpenSpec compatibility parser and its v1.6.0 oracle fixtures are unchanged. Separate tests cover the house rules.
- The cli's own specifications use MUST. `spec-lifecycle` splits off `spec-interop`, and `commit-attribution` splits off `worktree-identity`, so each capability stays under the limit.
- The scaffolded delta template uses MUST.

## Capabilities

- spec-lifecycle (modified)

## Impact

- `rune-docs/src/spec/{lint.rs,validate.rs,doctor.rs}`, `rune-docs/templates/spec/delta-spec.md`, `rune-docs/tests/fixtures/spec/*`.
- `docs/specs/{spec-lifecycle,spec-interop,commit-attribution,worktree-identity}/spec.md`, every active delta, `docs/walkthroughs/Spec.md`.
- Consumers that lint their specifications with this release convert SHALL to MUST and split any capability over the limit.
