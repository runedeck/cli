Refuse to sign a head the remote already holds, and name the immutable refusal, the pinentry trap, and the publish command.

## Plan

Signing in place rewrites the commit. On 2026-09-20 two heads reached `main` unsigned and were signed afterwards, so each publication became a sideways force-push of a protected tip, one after `--ignore-immutable`, and one attempt timed out while the key touch went to another window. The queue had the retry rule but never looked at the remote. One question before a request is recorded, and three clearer messages, close that.

## Changes

- Add `refuse_published_head` to `qualify_head` in `src/cli/sign/queue/mod.rs`: a head request is refused when `origin/<bookmark>` points at the head or contains it, error code `sign.head_published`.
- Add `SignOutcome::Immutable` in `src/cli/sign/queue/repo.rs` and stop retrying it in `src/cli/sign/queue/ceremony.rs`, with a reason that says the head is published.
- Print `TIMEOUT_HINT` on every pinentry timeout in `src/cli/sign/queue/ceremony.rs`.
- Print `publish with: jj git push --remote origin --bookmark <bookmark>` after a signed head, and carry it as `push` in `--json`, in `src/cli/sign/queue/mod.rs`.
- Extract the signing proof fixture into `docs/proofs/signing-fixture.sh`, give it a bare `origin` with `main` at the base, and source it from both signing proofs.
- Add `docs/proofs/sign-before-publish-guard/` (record.sh, cast, gif, transcript) and `docs/changes/sign-before-publish-guard/` with proposal, delta spec, tasks, and `adr.md`.
- Add one `CHANGELOG.md` line under Unreleased.

## Testing

- [x] `cargo test --all-features`: 791 unit tests and every integration suite pass, including the new `a_request_refuses_a_head_the_remote_already_holds`.
- [x] `cargo clippy --all-features --all-targets -- -D warnings` and `cargo fmt --check` clean.
- [x] `prek run --all-files` and `prek run --stage pre-push --all-files` in an isolated clone, both exit 0.
- [x] `docs/proofs/sign-before-publish-guard/record.sh` runs its six scenes to exit 0 against the built binary, and the recording is committed.

## Release Notes

- Refuse `rune sign queue` on a head that `origin` already holds, because signing it in place would rewrite a pushed tip.
- Report jj's immutable-commit refusal without a retry, print the pinentry focus hint on every timeout, and print the publish command after a signed head.
