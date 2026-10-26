## 1. QA pipeline (parity core)

- [x] 1.1 `suite.rs`: serde types, validation, suiteId derivation. Tests ported from suite.test.ts
- [x] 1.2 `scoring.rs`: is_correct with the 11 upstream cases
- [x] 1.3 `cache.rs`: signature, SHA-1 hash, cache write, gather/resume, mismatch guards. Tests from cache.test.ts
- [x] 1.4 `registry.rs`: models.yaml parsing, hard errors, env expansion. Tests from registry.test.ts
- [x] 1.5 `report.rs`: results/markdown/summary outputs. Tests from report.test.ts
- [x] 1.6 `run.rs`: plan, reuse, worker pool, timeout. Tests from run.test.ts
- [x] 1.7 Models: echo, ollama, openai-compatible, and the claude/codex/agy/grok/opencode coding tools through the `rune run` tool layer

## 2. Commands

- [x] 2.1 `rune bench run|report|list` in clap with suite-name resolution over tiers
- [x] 2.2 Bench workspace resolution: the `bench` config list and discovery, suites/{,user/,private/} tiers, results in the owning checkout
- [x] 2.3 `rune bench doctor`: workspace, registry, model readiness
- [x] 2.4 `rune bench audit`: self-score, negative collisions, short tokens. Judged suites skipped

## 3. Judged suites and dashboard

- [ ] 3.1 Judged suite schema, artifacts, checks, judge, human scores (the bun harness runs judged suites until then)
- [x] 3.2 `rune bench dashboard` from the workspace template

## 4. Parity and tier name

- [x] 4.1 Cross-runner parity: both runners on one suite with echo, results/summary/cache diffs clean except timestamps
- [x] 4.2 Cross-runner resume: bun cache resumed by rune and the reverse
- [ ] 4.3 Held-out tier named `suites/private/` in runedeck/bench and N4M3Z/bench (runedeck/bench main still documents `suites/sealed/`)

## 5. Verification

- [x] 5.1 cargo fmt, clippy -D warnings, full test suite green
- [x] 5.2 `docs/Manual Testing.md` bench section and `docs/walkthroughs/Bench.md`
