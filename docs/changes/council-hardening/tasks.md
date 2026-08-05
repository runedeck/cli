## 1. Fail-open and data-loss fixes

- [x] 1.1 Deploy manifest load: NotFound-only empty, fail closed otherwise
- [x] 1.2 Prune ownership fails closed on missing/unreadable provenance
- [x] 1.3 drift --target: missing deployed file is a failure (and Unreadable is a distinct drifted status; build sidecars and disabled providers scoped out)
- [x] 1.4 Sidecar names encode full filename + migration on read
- [x] 1.5 Tree re-adoption guard: digest + review-status checks
- [x] 1.6 dry-run planning creates no directories
- [x] 1.7 Symlink hardening: sidecar destinations, assembly sources, adopt walkers (finalize also requires SKILL.md to survive review)
- [x] 1.8 Finalize writes the sealed record before sidecar mutation
- [x] 1.9 Non-UTF-8 companions pass through as bytes end-to-end: assemble (byte-verbatim write, bytes provenance), deploy (bytes fingerprints and status), drift (byte compare when either side is binary), doctor (bytes verify and repair); build sidecars share the full-filename naming so same-stem assets stay distinct
- [x] 1.10 Strays: expand_tilde("~"), reserved suite id "cache", unknown assembly rules error, schema load errors propagate, reference stripping label-scoped

## 2. Bench fixes (coordinated with the bun harness where shared)

- [x] 2.1 Resume dedup per execution, both runners + compat spec note (cross-runner increased-runs resume verified live)
- [x] 2.2 Nonzero exit on errored runs, both runners
- [x] 2.3 runs/concurrency must be >= 1 (registry + flags)
- [x] 2.4 run_process: stdin written from a thread, readers started first
- [x] 2.5 --json for bench run/report/audit/dashboard
- [x] 2.6 Suite semantic validation (nonempty tests/prompts/answers)
- [x] 2.7 doctor probes per enabled model, env expansion deferred so list/doctor survive missing variables
- [x] 2.8 Worker results via channel, no lock poisoning
- [x] 2.9 grok prompts via --prompt-file in both runners; agy exposes no file/stdin input, argv documented

## 3. Shared infrastructure

- [ ] 3.1 One path-confinement helper (adopt, deploy, doctor) — adopt's contained_path hardened and sidecar resolvers centralized in manifest; deploy/doctor still carry their own copies
- [ ] 3.2 One atomic-replace API used by all writers — config::write_atomic is the canonical one; review-record and bench writers not yet migrated
- [x] 3.3 Atomic assemble: staging tree + swap, previous build restored on a failed swap
- [x] 3.4 Target lock for install/deploy/doctor --repair, with an exclusivity test
- [x] 3.5 Exit-code contract documented (docs/Exit Codes.md)
- [x] 3.6 Regression tests: increased-runs resume, tree re-adopt over local edits, dry-run no-write, prune ownership, unreadable manifest, sidecar naming + legacy fallback, lock exclusivity

## 4. Surface (additive)

- [x] 4.1 Deploy verb family + health ladder documented (docs/Command Map.md)
- [x] 4.2 import/adopt cross-referenced as one-shot vs reviewed modes (docs/Command Map.md)
- [x] 4.3 Personal ontology defaults out of the binary; only ~/Agents workshop defaults remain, machine values live in ~/.config/rune/config.yaml
