---
adr: docs/changes/openspec-import-destination/adr.md
status: proposed
decisions: ["Import writes the native root, and the doctor stays out of sibling checkouts"]
---

# Openspec import destination

## Why

Moving the html-tools tree with `rune spec import --openspec` converted 38 files into `openspec/` itself: with no `docs/` tree and no `spec.root`, autodetection made `openspec/` the root, so the import's destination was its source. It left an `.interop/` mirror and a lock inside the tree and moved nothing. In the same pass `rune adopt doctor` in the skeleton reported three records as moved, because it had walked into `.workspaces/fable-skeleton-fixes/` and read that checkout's sidecars against this checkout's paths.

## What Changes

- `import_openspec_with_io` resolves the destination to `docs` when the root was autodetected as `openspec/`. A configured `spec.root: openspec` keeps the in-place ownership mode.
- `spec_root_is_configured` on the spec API tells the two apart.
- The adoption doctor's `SKIP_WALK` gains `.workspaces`, `.worktrees`, and `.codex-prs`, the names the source snapshot already excludes.

The first live `rune sign open` on cli #67 signed the seal and then failed to push: the pinned transport re-adds only system- and user-scope helpers, read through the bare `credential.helper` key, so the URL-scoped helper `gh auth setup-git` writes was dropped and the repository-scope one was rightly ignored. The seal became a signed orphan. Now the push names `gh auth git-credential` for its own process (the same login `open` already used to read and ready the draft), URL-scoped helpers count, and `open` checks the transport before the key touch. The first ready flip then failed in CI: `verify-seal` read the pull request as `number` while the seal writes `pull_request`. Both spellings verify now.

## Capabilities

### New Capabilities

- `openspec-import-destination`: import from an autodetected `openspec/` root lands in `docs/`, and the adoption doctor skips sibling checkouts.

## Impact

- `rune-docs/src/interop/mod.rs`, `rune-docs/src/spec/{mod,root}.rs`, `src/cli/adopt/review.rs`, one unit test each.
