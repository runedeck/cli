Cap requirement, step, and changelog prose, floor change and capability names at three words, and bring this repository under the caps.

## Plan

Twelve requirement statements ran past 100 words, the changelog carried 55 entries over 200 characters, and 21 change ids and 35 capabilities had fewer than three words. The file-length rule never saw a paragraph and nothing linted the changelog or the names. The caps live in the rune checkers every repository already runs, so one binary carries them everywhere, and this repository is rewritten to pass them.

## Changes

- Add `lint_prose` to `rune-docs/src/spec/lint.rs`: a requirement statement over 100 words and a step over 30 words are errors in canonical specs and deltas.
- Add `lint_names` to `rune-docs/src/spec/lint.rs`: a change id or capability name under `spec.min_name_words` hyphen-separated words is an error, default 3, `0` off, read from the merged config through `set_name_rule_lookup`.
- Add `rune-docs/src/changelog.rs` and wire it into `rune docs check` in `src/cli/docs/mod.rs`: Keep a Changelog structure, one line per change of at most 200 characters, verb first, no `type:` prefix, no em-dash.
- Add `spec.min_name_words` to `SpecConfig` in `src/cli/config/source.rs` and regenerate `docs/config-reference.json`.
- Split 13 requirement statements in `docs/specs/model-commit-attribution/spec.md` and the `codex-skill-identity`, `sealed-review-ceremony`, and `owner-signing-queue` deltas, every MUST and scenario kept.
- Rewrite `CHANGELOG.md` from 175 entries to 343 one-line changes, `**Breaking:**` on the `--repair` removal.
- Rename 21 change directories and 35 capability directories to three words, and `docs/proofs/signing-queue/` to `docs/proofs/owner-signing-queue/`, with every reference updated.
- Add `docs/proofs/prose-length-caps/` (seven scenes) and `docs/changes/prose-length-caps/` with proposal, delta spec, tasks, and `adr.md`.

## Testing

- [x] `cargo test --all-features`: every suite passes, including 11 lint tests, 7 changelog tests, and `committed_reference_matches_the_binary`.
- [x] `cargo clippy --all-features --all-targets -- -D warnings` and `cargo fmt --check` clean.
- [x] `prek run --all-files` and `prek run --stage pre-push --all-files` in an isolated clone of `c520f6f7`, both exit 0.
- [x] `rune spec validate` and `rune spec doctor` from this build print no error for this repository and for the deck's `adopt-prose-length-caps` branch.
- [x] `docs/proofs/prose-length-caps/record.sh` runs its seven scenes to exit 0 against the built binary.

## Release Notes

- Refuse a requirement statement over 100 words and a scenario step over 30 words in `rune spec validate` and `rune spec doctor`.
- Refuse a change id or capability name under `spec.min_name_words` words (default 3) in `rune spec validate` and `rune spec doctor`.
- Lint `CHANGELOG.md` in `rune docs check`: Keep a Changelog groups in order, one line per change of at most 200 characters, verb first.
- Rename 21 change ids and 35 capabilities in this repository to three words.
