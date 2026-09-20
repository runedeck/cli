---
adr: docs/changes/repository-hygiene-sweep/adr.md
status: proposed
decisions: ["A consumer keeps its template pin current and its tree free of leftovers"]
---

# Repository hygiene sweep

## Why

The repository root held `openspec/project.md`, a stub from a one-time `rune spec export --openspec`, and `rune-docs/openspec/` held two forge-era proposals that no test or command reads. The Copier pin sat five commits behind the skeleton, so the `rune docs check` hook never fired on a changelog-only commit here. Two build directories beside `target/` (`target2`, `target-lint`, 10 GB together) were untracked but not ignored.

## What Changes

- Remove `openspec/project.md` and `rune-docs/openspec/`.
- Move the Copier pin to skeleton `64e7d2c`, which brings the `rune-docs-check` hook trigger `^(docs/|CHANGELOG\.md$)`. This repository keeps its own `REQUIRE_RUNE` form of the hook, because its CI builds `rune` from the same commit.
- Ignore `/target-*/` and `/target[0-9]*/`.

## Capabilities

### New Capabilities

- `repository-hygiene-sweep`: the tree carries no leftover spec exports, its Copier pin matches the skeleton main it last adopted, and every build directory is ignored.

## Impact

- `.gitignore`, `.pre-commit-config.yaml`, `answers.yaml`, and the two removed trees. No code.
- `.editorconfig`, `.cursor/BUGBOT.md`, and `review-cursor.yaml` exist in the skeleton and not here. They were dropped before the previous pin, so Copier does not restore them. A later change decides whether this repository wants them.
