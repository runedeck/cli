---
adr: "docs/decisions/ADR-XXXX.md"
status: proposed
---
# OpenSpec Artifact Parity

## Why

`rune spec` shares OpenSpec's delta dialect (ADDED/MODIFIED/REMOVED requirements with WHEN/THEN scenarios) but scaffolds a different artifact set, so a change authored under one tool cannot round-trip through the other. Verified against OpenSpec 1.6.0 (`@fission-ai/openspec`): rune omits `design.md`, its proposal carries no Capabilities section (the capability exists only as a CLI flag, exactly one per change), and `archive --abandon` rejects `-y`, which breaks the documented smoke script and any non-interactive ceremony runner.

## What Changes

- `rune spec propose` accepts repeated `--capability` flags and scaffolds one delta spec per capability.
- The proposal template gains a `## Capabilities` section (New/Modified lists) mirroring OpenSpec's, generated from the flags, so the proposal document itself declares which specs the change touches.
- `rune spec propose --design` (or a config default) scaffolds `design.md`; `rune spec context` includes it in the work order when present.
- `archive` accepts `-y` alongside `--abandon` as a no-op confirmation, so one flag convention works across merge and abandon paths and the smoke script runs as written.
- Compatibility note recorded in docs: rune roots at `docs/changes/` by design; OpenSpec 1.6.0 hardcodes `openspec/` (`OPENSPEC_ROOT_DIR` constant), so root parity is intentionally NOT pursued; parity is at the artifact and dialect level.

## Impact

- `spec propose`, `spec archive`, `spec context` commands; the proposal template; the spec-lifecycle smoke doc.
