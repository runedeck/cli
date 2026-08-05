---
adr: "docs/decisions/CLI-0022 Native bench runner.md"
status: proposed
---
# Harness Wiring

## Why

`rune install` deploys `rules/*.md` into every provider tree, but only Claude
Code reads a rules directory. Empirical probes and each harness's primary
documentation confirm: codex loads only its AGENTS.md chain (and reserves
`~/.codex/rules/` for `.rules` command policies), gemini-cli reads only the
GEMINI.md hierarchy, headless Antigravity currently loads no file context at
all, and opencode loads only AGENTS.md plus the `instructions` array in its
config. Deployed rules therefore reach one harness out of four; the others
depend on a per-machine forge-provision script that goes stale the moment a
rule changes. Tracks N4M3Z/forge-cli#92 with the council's corrections.

## What Changes

- Install gains a wiring step for home-scope targets: a marker-delimited
  generated block carrying the assembled rules is maintained in
  `~/.codex/AGENTS.md` and `~/.gemini/GEMINI.md`, and the rules glob is
  ensured in the `instructions` array of
  `~/.config/opencode/opencode.json`. Regenerated on every install;
  content outside the markers is never touched.
- The legacy forge-provision `harness-rules` block is replaced by the rune
  block on first run so exactly one generated region exists.
- Markdown rules stop deploying to `~/.codex/rules/` (a codex-owned
  namespace for command-policy files).
- Each block's hash is recorded in the provider manifest under a virtual
  key so drift and doctor can flag a stale block.
- `rune doctor` reports unwired or stale rule wiring per harness, including
  the headless-Antigravity limitation.
- Project-level installs are not wired (a repo's AGENTS.md belongs to the
  repo); doctor names the limitation.

## Capabilities

- deploy (modified)

## Impact

- rune: deploy wiring module, doctor findings, provider config for codex
  rules routing, tests.
- forge-provision: `scripts/configure/harness-rules.sh` becomes redundant
  once this ships.
- docs: Manual Testing and Command Map gain the wiring behavior.
