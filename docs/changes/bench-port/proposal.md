---
adr: "docs/decisions/CLI-0022 Native bench runner.md"
status: proposed
---
# Bench Port

## Why

The rune ecosystem's benchmark harness lives in runedeck/bench as a bun/TypeScript
runner. Bench is a component of rune, and the rune CLI is a single Rust binary with
no runtime dependencies; carrying a bun toolchain requirement for one subcommand
breaks that install story. The linked ADR records the decision to port the runner
natively into rune.

## What Changes

- `rune bench` becomes a native command family: `run`, `report`, `list`,
  `dashboard`, `doctor`, `audit`.
- The runner reimplements the SkateBench-compatible protocol documented in
  runedeck/bench `docs/skatebench-compat.md`, byte-compatible with the existing
  cache, results, and summary formats so prior results stay resumable.
- Suite tiers are discovered from a bench workspace: committed `suites/`,
  local `suites/user/`, and `suites/private/` (renamed from `suites/sealed/`)
  from a private downstream checkout, wired by config with autodetection.
- The bun harness stays in runedeck/bench as the reference implementation until
  parity is verified empirically, then retires.

## Capabilities

- bench (new)

## Impact

- rune: new `src/cli/bench/` module tree, new dependencies for HTTP and SHA-1.
- runedeck/bench: `suites/sealed/` renamed to `suites/private/`; README, push
  guard, and downstream runbook updated; harness marked as reference.
- N4M3Z/bench (private downstream): tracked sealed content moves to
  `suites/private/`.
- docs: Manual Testing gains a bench section; walkthroughs gain Bench.md.
