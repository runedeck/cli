## 1. Implementation

- [ ] 1.1 Workshop-root config key; mode detection under it; `--workshop` flag elsewhere
- [ ] 1.2 Step engine: idempotent steps, `--dry-run`, created-paths record, no auto-commit
- [ ] 1.3 Layout + git + jj colocate + hooks steps (consent-gated where they act on push or capture)
- [x] 1.4 `.rune` schema v2 `dirs:` (path, role, required; v1 reader retained; .rune.local overlay pending)
- [ ] 1.5 Vault mount association; satellites behind `--vault`, `--data`, `--remote`
- [ ] 1.6 `--spine` flag for non-workshop projects, gated on jj and entire presence

## 2. Verification

- [ ] 2.1 Tests: mode detection, idempotence (double-run), dry-run writes nothing, v1/v2 round-trip, consent refusal paths
- [ ] 2.2 cargo fmt, clippy, full suite; council review of the phase diff
