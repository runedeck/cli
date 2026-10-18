# Design for Sign before publish guard

## Context

The [proposal](proposal.md) states the scope and the [draft decision record](adr.md) the reasoning. The
[delta specification](specs/sign-before-publishing-online/spec.md) states the guard in role terms: the queue refuses a
published head, the signer names the immutable refusal, the pinentry trap, and the publication command. This file maps
each requirement to the command, flags, and strings that implement it.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids and messages | Exit status |
|---|---|---|---|---|
| A head request is refused once the remote holds the head | `rune sign queue <bookmark> --receipt <file>` compares the head with `refs/remotes/origin/<bookmark>`. The manual override is `jj sign -r <bookmark> --ignore-immutable` and a leased force-push, run by the owner. | none | `sign.head_published`: ``<bookmark> at <commit> is already published on <remote>: signing it in place would rewrite a pushed tip. Sign before you push. To re-sign a published head anyway, run `jj sign -r <bookmark> --ignore-immutable` yourself and force-push under a lease`` | nonzero, no request recorded |
| The signer names the immutable refusal and the pinentry trap | `rune sign next`. The immutable refusal is `jj sign` output that contains `is immutable`. The publication command is `jj git push --remote origin --bookmark <bookmark>`, printed as `publish with: ...` after `signed <bookmark> <commit>`, and carried as the `push` field under `--json`. | none | The reason is `refused: <bookmark> is immutable, which for a pushed bookmark means the head is published. Sign before you push.` The timeout hint is `Keep the pinentry focused and touch the key as soon as it appears; a touch while another window has focus types an OTP into that window.` | nonzero on the refusal, 0 after a signed head |
