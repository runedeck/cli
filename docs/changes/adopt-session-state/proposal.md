---
adr: "docs/decisions/CLI-0027 Temporary Adoption Session State.md"
status: proposed
---

# Adopt Session State

## Why

The adoption review state machine currently commits its entire block ledger beside provenance sidecars. That record is temporary workflow state, not final source provenance, and makes large reviewed imports produce large permanent files while doctor and reseal incorrectly depend on the duplicate ledger authority.

## What Changes

- **External pending sessions**: keep block text, verdicts, notes, flags, transports, and timestamps in crash-safe state outside the module tree, using worktree-specific Git metadata when available and a canonical-root-keyed user state/cache fallback otherwise.
- **Concise reviewed sidecars**: finalize writes final subject digests, reviewed state, reviewer, completion time, and a compact adaptation summary into adopt/v1 sidecars, then deletes the temporary session.
- **Sidecar-based health**: doctor verifies pending sessions and reviewed sidecar-to-file integrity, and reports legacy review ledgers with actionable inspection and explicit removal/archive instructions.
- **Sound reseal**: reseal selects a reviewed adopted artifact and updates its sidecar digests after maintainer touch-ups while refusing pending or unreviewed inputs.
- **Explicit legacy handling**: doctor identifies redundant legacy ledgers and directs maintainers to inspect and remove or archive them explicitly; no command silently deletes user files.

## Capabilities

- adopt-session-state (new; supersedes the permanent review-record requirements in adopt-review and adopt-hardening)

## Impact

- `src/cli/adopt/review.rs`, `src/cli/mod.rs`, provenance serde/generation, and adoption/provenance tests.
- `docs/changes/adopt-review`, `docs/changes/adopt-hardening`, adoption walkthrough and command documentation.
- Existing reviewed sidecars remain deployable without migration; legacy ledgers are diagnosed but never silently deleted.
