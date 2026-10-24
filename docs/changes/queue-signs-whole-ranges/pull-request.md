Queue a whole range for an owner direct push, and stop a bookmark named after a commit id from redirecting the key.

## Plan

A direct push to `main` needs the owner's signature on every commit it adds, and the queue signed one commit per request, so a 77-commit stack was signed by hand with copied pins. One range request covers the stack now. While reading the signer, the bare commit id it passed to `jj sign -r` turned out to resolve through bookmarks, tags, and aliases first, so a session could pick what the key signs. Every `jj sign` now goes through `commit_id()`.

## Changes

- Add `--range` to `rune sign queue` in `src/cli/mod.rs` and `queue_range` in `src/cli/sign/queue/mod.rs`: one `range` request for `git rev-list origin/<protected>..<head>`, refused when empty (`sign.range_empty`) or published.
- Add `sign_range` and `verify_signed_range` in `src/cli/sign/queue/mod.rs`: show every commit, read `y`, sign in one `jj sign` run, check each commit's identity and `KEYS` signature, record the new head, and skip the key when every commit is already owner-signed.
- Make a range request stale in `derive` when a commit identity changes or the protected ref leaves the base, and check the protected ref again after the refresh.
- Name commits as `commit_id("<id>")` in `Repo::sign` in `src/cli/sign/queue/repo.rs`, and refuse while a revset alias redefines `commit_id`.
- Add state `moved` for a signed request whose bookmark moved: listed, skipped without failing, pruned.
- Add `RangeRequest` and kind `range` to `src/cli/sign/queue/store.rs`, and `range_view` to `src/cli/sign/queue/ceremony.rs`.
- Add seven integration tests to `tests/sign_queue.rs`, two unit tests, and the proof `docs/proofs/queue-signs-whole-ranges/`.
- Add `docs/changes/queue-signs-whole-ranges/` with proposal, delta spec, design, tasks, and `adr.md`, and three `CHANGELOG.md` lines.

## Testing

- [x] `cargo test --all-features`: 1767 tests in 39 suites pass. With the bare-id form back in place, `a_bookmark_named_after_the_commit_id_does_not_redirect_the_key` fails and the key signs the bookmark's target. Without the alias guard, `a_commit_id_alias_is_refused_and_nothing_is_signed` fails.
- [x] `cargo clippy --all-features --all-targets -- -D warnings` and `cargo fmt --check` clean.
- [x] `rune proof run queue-signs-whole-ranges`: 11 of 11 scenes proven at `c47a52205528`, and `--check` holds.
- [x] `rune spec validate queue-signs-whole-ranges` passes.

## Release Notes

- Add `rune sign queue <bookmark> --range`, which queues every commit a direct push adds as one request that `rune sign next` lists, signs in one run, and checks against `KEYS`.
- Show a signed request whose bookmark moved as `moved`, so `rune sign next` and `all` no longer report it as stale.
- Name every commit the queue signs through `commit_id()`, so a bookmark, tag, or alias named after a commit id cannot move the owner's signature.
