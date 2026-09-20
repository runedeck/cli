Let `rune sign open` seal above an owner-signed head and read the body from the branch, and run the `rune-docs` unit tests in CI.

## Plan

Four rough edges from tonight's landings, each with a test: `open` refused a `jj sign`ed head and read the body from the working copy. `archive` wrote `TBD` purposes. `doctor` hid a parse error behind `no recognized requirements`. `promote` took the first provider copy silently. The fifth is the finding under them: `rune-docs` never ran its unit tests in CI, and five were failing.

## Changes

- Add `Repo::file_at` and read the body and `schemas/PULL_REQUEST.mdschema` from the bookmark's tree in `open`, and judge `adopt` bodies against the protected branch's schema.
- Accept an owner-signed head in `open`, refuse a signature from a key outside `KEYS`, document `--repo`.
- Take a new capability's purpose from the proposal in `archive`, report parse issues with their line in `doctor`, and refuse divergent provider copies in `promote`.
- Add `rune-docs` to the Cargo workspace, run `cargo test --workspace` in `test.yaml`, `release.yaml`, and the push hook, and fix the five broken unit tests, the two clippy findings, and the rustfmt drift.
- Update the `sealed-review-ceremony` delta and add `docs/changes/sign-open-branch-tree/`.

## Testing

- [x] `cargo test --workspace --all-features`: 1787 passed, including two new `sign_queue` cases, the purpose lookup, the doctor finding, and the divergent promote.
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` clean.
- [ ] Both prek stages in an isolated clone.

## Release Notes

- Change `rune sign open` to accept an owner-signed head and read the body from the branch, and run the `rune-docs` unit tests in CI.
