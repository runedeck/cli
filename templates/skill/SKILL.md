---
name: rune
description: "Drive the rune CLI to stage, deploy, and verify runes (skills, agents, rules) from a deck into AI harness directories. USE WHEN install runes, add skills to a project, deploy deck content, check drift, .rune manifest, rune validate, spec-driven change, rune spec. Not for authoring deck content (use the deck's build-* skills)."
version: ${VERSION}
---

# rune

rune deploys markdown instruction files (runes) from a deck into provider directories (`.claude`, `.codex`, `.gemini`, `.opencode`). The consumer manifest is `.rune` at the repo root; deployment records live in per-provider `.manifest` files.

## Orientation

Run `rune context --json` first: it reports the acting root and role, the target binding, the manifest selection, which providers are deployed, active spec changes, and suggested next steps. `rune status --source <deck>` is the deck-side dashboard.

## Consumer Flow

```sh
rune add development                 # stage a whole deck domain
rune add --cast development          # stage a cast (named selection)
rune add development/skills/deslop   # stage one rune by qualified id
rune install                         # assemble and deploy to all providers
rune drift --target .                # deployed content matches source?
rune doctor --target .               # integrity: ok / modified / missing / orphan
```

`rune add` acts on the bound target when the current directory has no `.rune`; it prints a note when it redirects. Ambiguous ids fail loudly listing every candidate; retry with the qualified form `<domain>/<kind>/<name>`.

## Safe Scripting

- Inspection commands (`context`, `status`, `doctor`, `drift`, `provenance`, `config`, `spec …`) accept `--json` for structured output; staging commands (`add` and the kind adds) print plain text.
- `rune install --dry-run` previews pruning without writing.
- `rune doctor --repair` restores missing files and quarantines orphans; it never overwrites user edits. Overwriting edits takes a deliberate `rune install --force`.
- Never edit deployed files under provider directories; edit the deck source and reinstall. Local exceptions live in a rune's `user/` override.

## Spec Lifecycle

```sh
rune spec propose <change-id> --capability <name>
rune spec list                       # active changes; --specs for capabilities
rune spec show <id>                  # one change or capability spec
rune spec context <id>               # agent-ready work order
rune spec doctor                     # relationship health
rune spec archive <id>               # refuses on unchecked tasks; --abandon drops
```

## Validation

`rune validate` checks schemas and structure. Add `--scan` only in commit and push hooks; it runs gitleaks and semgrep and is slow.
