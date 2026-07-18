## 1. Implementation

- [ ] 1.1 Shared root resolver; every spec consumer routes through it
- [ ] 1.2 Compat mode (`spec.root: openspec` + autodetection; ambiguity errors)
- [ ] 1.3 `spec export --openspec` and `spec import --openspec` over a normalized change model

## 2. Verification

- [ ] 2.1 Golden round-trip tests both directions; ambiguity fixtures
- [ ] 2.2 cargo fmt, clippy, full suite; council review of the phase diff
