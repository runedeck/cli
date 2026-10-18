## 1. Implementation

- [x] 1.1 Move pending block records to worktree-specific Git metadata with canonical-module-root user state/cache fallback
- [x] 1.2 Extend adopt/v1 sidecars with concise final review metadata and crash-safe finalize ordering
- [x] 1.3 Rebase doctor and reseal on pending sessions and reviewed sidecars
- [x] 1.4 Diagnose legacy ledgers with an actionable explicit removal/archive path
- [x] 1.5 Preserve deploy and assembly behavior based on sidecar reviewed state

## 2. Verification

- [x] 2.1 Cover pending discovery, no-ledger finalize, worktree isolation, sidecar-only doctor, legacy diagnosis, reseal, and deploy regression
- [x] 2.2 Run cargo fmt, targeted tests, clippy, and the full test suite
- [x] 2.3 Update walkthrough, command map, ADR, OpenSpec deltas, and changelog
