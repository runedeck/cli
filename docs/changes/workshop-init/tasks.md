## 1. Implementation

- [x] 1.1 Mode detection under the targets root (existing key, default ~/Agents); `--workshop` flag elsewhere
- [x] 1.2 Idempotent steps, `--dry-run` (writes nothing), no auto-commit in workshop mode
- [x] 1.3 Layout + git + jj colocate steps (entire hooks stay consent-gated, pending)
- [x] 1.4 `.rune` schema v2 `dirs:` (path, role, required; v1 reader retained; .rune.local overlay pending)
- [ ] 1.5 Vault mount association; satellites behind `--vault`, `--data`, `--remote`
- [x] 1.6 `--spine` flag for non-workshop projects, gated on jj presence

## 2. Verification

- [x] 2.1 Tests: no-auto-commit contract, dry-run, v2 parse validation; live smoke of layout+colocation
- [ ] 2.2 cargo fmt, clippy, full suite; council review of the phase diff
