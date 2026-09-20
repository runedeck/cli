---
adr: docs/changes/sign-before-publish-guard/adr.md
status: proposed
decisions: ["Sign before publish, and the queue refuses a published head"]
---

# Sign before publish guard

## Why

On 2026-09-20 two heads reached `main` unsigned and were signed afterwards by hand with `jj sign`. Each signing rewrote the commit, so each publication became a sideways force-push of a protected tip (cli `4fa850b9` to `64fc7e85`, deck `50a17fae` to `88bdc3bd`), the second one only after `--ignore-immutable`, and one attempt died on a pinentry timeout while the key touch went to another window. `rune sign queue` had the retry rule and the pinned settings that would have avoided the last two, but nothing in it knew that the head was already published, so nothing said "you are about to rewrite a pushed tip".

## What Changes

- `rune sign queue <bookmark>` refuses a head that `origin/<bookmark>` already holds, or that sits beneath it, and names the manual override as the owner's own step.
- `rune sign next` classifies jj's immutable-commit refusal as a non-retried failure with the same message, prints the pinentry focus hint on every timeout, and prints the publication command after a signed head. It still pushes nothing.

## Capabilities

### New Capabilities

- `sign-before-publish-guard`: a head request is refused when the remote already holds the head. The signer names the immutable refusal, the pinentry trap, and the publication command.

## Impact

- `src/cli/sign/queue/{mod,repo,ceremony}.rs`, one unit test, one integration test in `tests/sign_queue.rs`.
- No change to `open`, `submit`, `next` on merge requests, or the store format. A queued request gains one optional `push` field in `--json` output.
