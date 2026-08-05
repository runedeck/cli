## Why

Rune changes have specifications, decisions, implementation work, tests, review, and delivery controls, but no shared lifecycle connects those phases or verifies that each transition reflects the maintainer's intent. A portable change workflow makes the complete path explicit while preserving human approval at every consequential boundary.

## What Changes

- Define the lifecycle from specification through decision artifacts, implementation, testing, pull-request review, and deployment.
- Add a tailored specification interview that reviews goals, non-goals, and scenarios through the harness's structured question function or a plain-text fallback.
- Apply interview revisions immediately, rerun strict validation, and revisit affected material before acceptance.
- Require explicit approval before decision drafting, implementation, publication, and deployment transitions.
- Make the lifecycle resumable from its existing artifacts and recorded phase state.
- Treat deployment as arrival at the intended published destination, including merged documentation visible on GitHub.

## Capabilities

### New Capabilities

- `development-lifecycle`: Defines the portable, review-driven process that carries a change from specification to deployment.

### Modified Capabilities

None.

## Impact

- Adds a core development skill that coordinates existing specification, decision, implementation, testing, review, and delivery functions.
- Extends the project workflow with a specification-review checkpoint before decision artifacts and implementation tasks.
- Requires harness adapters to use their native structured question function when available.
- Adds lifecycle state and validation needed to resume interrupted work without repeating accepted phases.
