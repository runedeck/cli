# Consumers Copier Design

## Approach

Rewrite the phantom pin to the deck's real baseline, let Copier merge to the current skeleton commit, then re-add the cli's own build targets, test step, and lint excludes on the template side of every conflicted file. Prose corrections are mechanical and position-bound.

## Structure

- `answers.yaml`, `Makefile`, `.github/workflows/quality.yaml`, `.pre-commit-config.yaml`: template base plus cli additions.
- `.rumdl.toml`, `typos.toml`: template base plus cli excludes and accepted words.
- `.github/workflows/{release,test,module-release}.yaml`: cli-only workflows hardened to the template's zizmor baseline.
- `docs/changes/consumers-copier`: this change.

## Risks

- The cli additions conflict again on the next template change to the same file. They are small and listed above.
- `rune-docs/` is a vendored crate with its own openspec tree. Its prose is corrected here because the Vale hook covers it. Its spec tree is not this repository's.
