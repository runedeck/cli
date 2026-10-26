---
adr: "docs/decisions/CLI-0022 Native bench runner.md"
status: proposed
decisions: ["Native bench runner"]
---

# Benchmark runner in rune

## Why

The rune CLI installs as one Rust binary with no runtime dependencies. A benchmark runner that needs bun and a second checkout cannot live inside that binary, and a bun toolchain for one subcommand breaks the install story. The suite format comes from SkateBench ([T3-Content/skatebench](https://github.com/T3-Content/skatebench)), and runedeck/bench `docs/skatebench-compat.md` specifies the cache, results, and summary formats byte for byte. A small, fully specified protocol can run natively in rune and keep the data that the bun harness already wrote. CLI-0022 records why rune runs the benchmark itself.

## What Changes

- `rune bench` is a native command family: `run`, `report`, `list`, `dashboard`, `doctor`, `audit`.
- The runner implements the SkateBench-compatible protocol of runedeck/bench `docs/skatebench-compat.md`. Its cache, results, and summary documents match the bun harness byte for byte, so either runner resumes from what the other wrote.
- Suites come from each bench checkout in three tiers: committed `suites/`, local `suites/user/`, and held-out `suites/private/`. The `bench` config key lists the checkouts, and rune looks at two discovery paths when the key is unset.
- Coding tool models (`claude-cli`, `codex-cli`, `agy-cli`, `grok-cli`, `opencode-cli`) run through the same arguments and reply parsing as `rune run`.
- The bun harness in runedeck/bench is the reference implementation that the parity check compares against.

## Capabilities

- benchmark-results-stay-compatible (new)

## Impact

- rune: the `src/cli/bench/` module tree, and dependencies for blocking HTTP and SHA-1.
- runedeck/bench and its private downstream N4M3Z/bench: the held-out tier directory is `suites/private/`, so their README, push guard, and runbook must name it.
- docs: `docs/Manual Testing.md` has a bench section, and `docs/walkthroughs/Bench.md` walks through the commands.
