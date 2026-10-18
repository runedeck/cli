---
adr: docs/changes/edge-cases-accompany-specs/adr.md
status: proposed
---

# Requirement levels and edge cases

## Why

Every requirement in this repository uses MUST, so every rule reads equally loud. The edge cases of one rule sit as scenarios right under it. A specification with twenty requirements and three scenarios each is too much text for one owner to read. An edge case is often where a real decision hides.

The owner asked in the review of 17 October 2026 for one file he has to read that covers the main points. The detail sits beside it where implementers find it, and he is pointed at the detail that changes a decision. The keyword levels of RFC 2119 (Request for Comments 2119, the Internet standard for requirement keywords) give the levels. MoSCoW (must have, should have, could have, will not have) is the planning form of the same scale. This change's record amends CLI-0040 Specification House Rules, which chose one keyword and left the parser's keyword rule unchanged.

## What Changes

- A requirement states its level with MUST, SHOULD, or MAY. The validator keeps reporting `SHALL`.
- Each requirement keeps exactly one primary scenario in the specification. The validator reports a second one, as a warning while the repository migrates.
- Edge cases move to a companion file beside the specification. Each is a titled block with WHEN and THEN. A case that narrows, overrides, or contradicts its requirement carries a decision line.
- The proof runner covers every edge case the way it covers every scenario.
- The review view the owner reads is generated on demand from the specification and the marked edge cases. It is never committed, and the validator reports a committed one.
- The compatibility parser keeps reading the specification unchanged, and the companion is invisible to OpenSpec. The parser's keyword rule widens to the three levels, which is the one divergence from OpenSpec this change adds.
- This change uses its own shape: one scenario per requirement in `spec.md`, the rest in `edge-cases.md`.

## Capabilities

### New Capabilities

- `edge-cases-accompany-specs`: three keyword levels, one primary scenario per requirement, edge cases in a companion, proofs over edge cases, and a generated review view.

## Impact

- `rune-docs/src/spec/{lint.rs,parse.rs,apply.rs}`, the proof runner and its scene writer, a new review view generator, the delta template, their tests.
- Every active change: scenarios past the first move into companions during the migration. The archiver carries companions into the canonical tree.
- The deck's review skill can read the review view instead of the raw specification.
