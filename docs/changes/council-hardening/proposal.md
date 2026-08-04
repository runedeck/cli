---
adr: "docs/decisions/CLI-0022 Native bench runner.md"
status: proposed
---
# Council Hardening

## Why

A three-seat cross-vendor review of the whole CLI surfaced a set of verified
defects concentrated in fail-open error paths (unreadable state treated as
empty), overwrite guards that check existence instead of content, symlink
handling, and benchmark resume accounting, plus a slate of robustness and
surface improvements. Every item below was confirmed against the source
before being scheduled.

## What Changes

Bug fixes:

- Deploy treats only a missing `.manifest` as empty; read errors fail closed.
- Prune refuses ownership when provenance is missing or unreadable.
- `drift --target` fails when a deployed file is missing.
- Provenance sidecar names encode the full filename, with migration for
  existing stem-named sidecars.
- Tree re-adoption applies the same local-edit and review-status guards as
  single-file adoption.
- `--dry-run` planning never creates directories.
- Symlinks are rejected in adopt sidecar destinations and assembly sources,
  and surfaced (not dropped) by the adopt walkers.
- Adoption finalize writes the sealed record before mutating sidecars.
- Non-UTF-8 companions pass through assembly as bytes.
- Bench: resume deduplicates per-execution candidates (both runners), errored
  runs exit nonzero (both runners), `runs`/`concurrency` must be positive,
  subprocess stdin is written from a thread so large prompts cannot deadlock.
- Strays: `~` expands alone, suite id `cache` is reserved, unknown assembly
  rules and unreadable custom schemas are hard errors, reference stripping
  only removes defined labels.

Improvements:

- `--json` honored by bench run/report/audit/dashboard and import/adopt.
- One path-confinement helper, one atomic-replace API, atomic assemble into a
  temp tree, and a lock on install/deploy/doctor-repair targets.
- Documented exit-code contract; suite semantic validation; per-model bench
  doctor probes; channel-based bench worker harvesting; file-based prompts
  for argv-limited providers.
- Surface (additive only): the deploy verb family documented around
  `install`, the health ladder documented, import/adopt cross-referenced as
  one-shot vs reviewed modes, personal ontology defaults moved to config.

## Capabilities

- integrity (modified)
- bench (modified)

## Impact

- rune: src/cli/{deploy,drift,adopt,assemble,bench,validate}, src/manifest,
  src/ontology.rs, new shared helpers under src/cli/config or src/services.
- runedeck/bench: bun harness gains the coordinated resume-dedup and
  exit-code changes; compat spec gains both.
- docs: exit-code contract and health-ladder sections; Manual Testing
  updates where behavior changes.
