---
adr: docs/changes/prose-length-caps/adr.md
status: proposed
decisions: ["Prose has a cap the checker enforces"]
---

# Prose length caps

## Why

Twelve requirement statements in this repository ran past 100 words, the longest at 354, and the changelog carried 55 entries over 200 characters, several over 1,400. Both read as walls: a requirement that is really five, a changelog line that is a release note. The existing `spec-too-long` rule caps a file at 150 lines and never sees a single paragraph. Nothing lints the changelog at all.

## What Changes

- `rune spec validate` and `rune spec doctor` report an error for a requirement statement over 100 words and a scenario step over 30 words, in canonical specs and deltas.
- `rune spec validate` reports an error for a change id or capability name under `spec.min_name_words` hyphen-separated words (default 3, `0` off). The deck passes at 3. This repository renames 21 change ids and 35 capabilities to pass, and `docs/proofs/signing-queue/` becomes `docs/proofs/owner-signing-queue/`.
- `rune docs check` lints `CHANGELOG.md`: Keep a Changelog 1.1.0 structure (title, `[Unreleased]` first, `[X.Y.Z] - YYYY-MM-DD` newest first, the six groups in order) and Common Changelog line discipline (one line per change, at most 200 characters, verb first, no `type:` prefix, no em-dash).
- The thirteen requirement statements and the changelog in this repository are rewritten to pass.

## Capabilities

### New Capabilities

- `prose-length-caps`: the spec lint caps requirement statements and steps by word count and floors change and capability names by word count, and the docs check caps changelog entries by line and character count.

## Impact

- `rune-docs/src/spec/lint.rs`, `rune-docs/src/changelog.rs`, `src/cli/docs/mod.rs`, their tests.
- `docs/specs/model-commit-attribution`, four change deltas, and `CHANGELOG.md` in this repository. The deck has one canonical spec, three deltas, and its changelog to fix under the same rule once its pinned rune moves.
- The `rune-docs-check` hook's `files:` pattern in each repository should include `CHANGELOG.md`. Today it fires on `docs/` alone, so a changelog-only commit is checked in CI's all-files run and not locally. That is a skeleton change.
