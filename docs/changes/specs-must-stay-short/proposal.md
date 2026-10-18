---
adr: "docs/decisions/CLI-0040 Specification House Rules.md"
status: proposed
---

# Spec House Rules

## Why

The skeleton's review-ceremony specification grew past three hundred lines, mixed SHALL with MUST, and defined its own terms in passing with nowhere to look them up. The parser accepts both keywords because OpenSpec artifacts use either, so the house style needs its own check. [CLI-0040](../../decisions/CLI-0040%20Specification%20House%20Rules.md) records the decision to keep compatibility in the parser and put the house rules in a separate lint.

## What Changes

- `rune spec validate` and `rune spec doctor` report a prose line that uses SHALL, a canonical specification over 150 lines (a delta over that length warns), and a term link that names no ontology term. The design's Commands table names the diagnostic ids.
- A defined term is a link to its generated glossary page, emphasized at the first use in a file and plain after it. Plain italics are emphasis. The change `every-term-cites-definition` carries the term rules in full.
- The OpenSpec compatibility parser and its v1.6.0 oracle fixtures are unchanged. Separate tests cover the house rules.
- The cli's own specifications use MUST. `lifecycle-of-a-change` splits off `openspec-trees-full-compatibility`, and `model-identity-on-commits` splits off `commits-carry-model-identity`, so each capability stays under the limit.
- The delta template uses MUST.

## Capabilities

- lifecycle-of-a-change (modified)

## Impact

- `rune-docs/src/spec/{lint.rs,validate.rs,doctor.rs}`, `rune-docs/templates/spec/delta-spec.md`, `rune-docs/tests/fixtures/spec/*`.
- `docs/specs/{lifecycle-of-a-change,openspec-trees-full-compatibility,model-identity-on-commits,commits-carry-model-identity}/spec.md`, every active delta, `docs/walkthroughs/Spec.md`.
- Consumers that lint their specifications with this release convert SHALL to MUST, split any capability over the limit, and run `rune spec fix --links` to convert their term marks.
