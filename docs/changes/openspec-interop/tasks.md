## 1. Implementation

- [ ] 1.1 Shared root resolver; every spec consumer routes through it
- [ ] 1.2 Compat mode (`spec.root: openspec` + autodetection; ambiguity errors)
- [x] 1.3 `spec export --openspec` and `spec import --openspec` (structural copy, overwrite refusal)

## 2. Verification

- [x] 2.1 Round-trip tests both directions; collision and missing-source fixtures
- [ ] 2.2 cargo fmt, clippy, full suite; council review of the phase diff
