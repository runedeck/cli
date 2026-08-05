## 1. QA pipeline (parity core)

- [x] 1.1 `suite.rs`: serde types, validation, suiteId derivation; tests ported from suite.test.ts
- [x] 1.2 `scoring.rs`: is_correct with the 11 upstream cases
- [x] 1.3 `cache.rs`: signature, SHA-1 hash, cache write, gather/resume, mismatch guards; tests from cache.test.ts
- [x] 1.4 `registry.rs`: models.yaml parsing, hard errors, env expansion; tests from registry.test.ts
- [x] 1.5 `report.rs`: results/markdown/summary outputs; tests from report.test.ts
- [x] 1.6 `run.rs`: plan, reuse, worker pool, timeout; tests from run.test.ts
- [x] 1.7 Providers: echo, ollama, openai-compatible, claude/codex/agy/grok/opencode CLI

## 2. Command surface

- [x] 2.1 `rune bench run|report|list` wired into clap with suite-name resolution over tiers
- [x] 2.2 Bench workspace resolution: bench.root config + discovery, suites/{,user/,private/} tiers, private results routing
- [x] 2.3 `rune bench doctor`: workspace, registry, provider readiness
- [x] 2.4 `rune bench audit`: self-score, negative collisions, short tokens; judged suites skipped

## 3. Judged suites and dashboard

- [ ] 3.1 Judged suite schema, artifacts, checks, judge, human scores (bun harness remains the judged runner until this lands)
- [x] 3.2 `rune bench dashboard` from the workspace template

## 4. Parity and rename

- [x] 4.1 Cross-runner parity: both runners on tier1-sample with echo, results/summary/cache diffs clean modulo timestamps
- [x] 4.2 Cross-runner resume: bun cache resumed by rune and vice versa, full reuse both directions
- [x] 4.3 sealed → private rename in runedeck/bench and N4M3Z/bench (staged in both repos)

## 5. Verification

- [x] 5.1 cargo fmt, clippy -D warnings, full test suite green
- [ ] 5.2 Manual Testing.md bench section + walkthroughs/Bench.md + PDF refresh
