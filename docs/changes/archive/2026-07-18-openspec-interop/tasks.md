## 1. Implementation

- [x] 1.1 Shared root resolver; every spec command routes through it
- [x] 1.2 Compat mode (`spec.root: openspec` + autodetection when only openspec/ exists)
- [x] 1.3 `spec export --openspec` and `spec import --openspec` (structural copy, overwrite refusal)

## 2. Verification

- [x] 2.1 Round-trip tests both directions; collision and missing-source fixtures
- [x] 2.2 cargo fmt, clippy, full suite green
