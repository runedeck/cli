# Tasks

## 1. Implementation

- [x] 1.1 `lint_prose` in `rune-docs/src/spec/lint.rs`: `MAX_REQUIREMENT_WORDS = 100`, `MAX_STEP_WORDS = 30`, error in canonical and delta
- [x] 1.2 `rune-docs/src/changelog.rs`: title, release headings, order, groups, one-line entries, verb first, no prefix, no em-dash, one notice
- [x] 1.3 `rune docs check` prints the changelog block and fails on its errors, `--json` carries `changelog`
- [x] 1.4 `lint_names`: `spec.min_name_words` (default 3, `0` off) through `set_name_rule_lookup`, wired from the merged config

## 2. Rewrite

- [x] 2.1 Split the thirteen over-long requirement statements in this repository (model-commit-attribution, codex-skill-identity, sealed-review-ceremony, owner-signing-queue)
- [x] 2.2 Rewrite `CHANGELOG.md` to the shape, every change kept as one line

## 3. Verification

- [x] 3.1 Unit tests for both rules
- [x] 3.2 `rune spec validate`, `rune docs check`, and both prek stages pass on this repository
- [x] 3.3 One recorded proof per scenario under `docs/proofs/prose-length-caps/`

## 4. Record

- [ ] 4.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
- [x] 4.2 Rename the 21 change ids and 35 capabilities under three words in this repository
- [ ] 4.3 Skeleton: `rune-docs-check` hook `files:` includes `CHANGELOG.md`
