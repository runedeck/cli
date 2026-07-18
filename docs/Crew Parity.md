# Crew Parity Matrix

Capability comparison against crew ("Homecrew"), pinned to with-logic/crew `main` @ `19c0369` (2026-07-15) [CREW]. Parity here means rune covers the capability its own way; explicit non-goals honor ASSEMBLY-0005: lockfile-style package management stays outside rune.

| Capability (crew @ 19c0369) | Rune | Status |
|------------------------------|------|--------|
| Source refs: tap names, `@owner/repo@ref//subpath`, local paths | `.rune` sources: `local:` paths and `git:` HTTPS pinned to a 40-char SHA | Covered differently: rune pins by commit in a committed manifest; ref grammar sugar is a non-goal |
| Registries ("taps"): any git repo, `skills/` indexing | Decks: structured repos with `deck.yaml`, domains, casts, validation | Covered: decks are rune's registry unit, richer than a bare skills dir |
| Resolved-SHA recording (`.crew.json` per install) | `.manifest` fingerprints per target + SLSA provenance sidecars per file | Covered: rune records more (hashes + provenance), in-tree |
| Dependency graph in frontmatter | Casts (named selections) express grouping; no transitive skill deps | Non-goal: deps between skills stay a deck-authoring concern |
| `crew update` + background autoupdate | `rune install` re-materializes pinned sources; `rune watch` monitors locations | Partial: no background scheduler by design (deploys are explicit) |
| Agent auto-detection, copy into each harness dir | Providers in `defaults.yaml` (claude, codex, gemini, opencode; agentskills opt-in) with per-provider assembly transforms | Covered: rune targets a curated provider set with format conversion, not blind copies |
| Generic fallback `~/.agents/skills/` | agentskills provider targets `.agents` (opt-in) | Covered (opt-in) |
| Content-hash conflict protection (`customized`, `--force`) | Manifest fingerprints: drift/doctor detect modification; install refuses without `--force` | Covered |
| Atomic rename install + advisory lock | Atomic temp-and-rename writes for state files; deploys are manifest-pruned | Partial: no cross-process advisory lock; single-operator tool by design |
| No symlinks, copies only | Same: every deploy is a file copy | Covered |

## Non-goals (per ASSEMBLY-0005)

- A project lockfile or npm-style dependency resolution between skills.
- Background autoupdate schedulers; deployment stays an explicit, reviewable action.
- Blind per-harness copies without format transformation; providers define assembly rules.

[CREW]: https://github.com/with-logic/crew "with-logic/crew README @ 19c036951c7bf1baece81b0f2304b7c29baef211"
