# Design for Signing Queue

## Context

The [proposal](proposal.md) states the scope. The delta specifications [queue-holds-signing-requests](specs/queue-holds-signing-requests/spec.md) and
[protect-signing-with-mutex](specs/protect-signing-with-mutex/spec.md) state the behavior in role terms: a session asks, the queue records and lists, the signer signs at the owner's key. This file
maps each requirement to the command, flags, paths, and strings that implement it in the rune command line tool, so
the specification never has to name them.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids and messages | Exit status |
|---|---|---|---|---|
| A session queues a signing request | `rune sign queue <bookmark> --receipt <file>` records a `head` request, signed in place. `rune sign submit <bookmark> --receipt <file>` is a separate command that records a `merge` request admitted against the controller's review scorecard, under [CLI-0042](../../decisions/CLI-0042%20Sealed%20Review%20Ceremony%20Commands.md) and the `owner-key-signs-merges` change. `--repo <dir>` names the workspace, default the enclosing jj workspace. | none | none | 0 when recorded |
| The queue hands the request to the owner | The handoff line names `rune sign next`. The notification goes through the desktop notifier. | none | none | 0 even when no notification can be sent |
| The queue refuses a request it cannot trust | `rune sign queue <bookmark> --receipt <file>`. `--receipt` is required. | none | The refusal names the missing bookmark, the signed head, the commit the receipt covers, or the existing request. | nonzero, nothing recorded |
| A receipt ends with an exit line | `--receipt <file>` whose last line is `exit=<status>` or `<stage>-exit=<status>`. | none | The refusal names the receipt. | nonzero, nothing recorded |
| The queue is one directory for every repository | `rune sign queue` lists, `rune sign queue --json` emits the array, `rune sign queue --prune` prunes. The queue directory is `$XDG_STATE_HOME/rune/sign-queue`, by default `~/.local/state/rune/sign-queue`. `RUNE_STATE_DIR` replaces the `rune` directory. | none | The empty listing and `rune sign next` name the queue directory. | 0 |
| The queue lists requests in stack order | `rune sign queue` with no bookmark lists. With a bookmark it queues that bookmark. `rune sign drop <bookmark>` and `rune sign queue --prune` remove a `stale` request. | none | States `stale` and `blocked` in the listing. | 0 |
| Each request shows one state | `rune sign queue`, `rune sign show <bookmark>`. | none | States `current`, `stale`, `blocked`, `claimed`, `unverified`, `signed`. | 0 |
| Signing an ancestor keeps a request current | `rune sign queue` after `rune sign next` signed the ancestor. | none | State `current`. | 0 |
| The owner signs from the queue | `rune sign next` signs the first request, `rune sign all` every one. The signing tool is `jj sign -r <commit id>`. A timeout is attempted `SIGN_ATTEMPTS` = 3 times in all, so at most two retries. | none | A skipped request is reported with its state. | nonzero when any request failed or was skipped |
| Each request is claimed and signed under the lock | `rune sign next` and `rune sign all` run `jj sign -r <commit id>` in the request's workspace. | none | The failure reports the request as stale. | nonzero on a moved bookmark |
| Each signed head is verified and recorded | `rune sign next` and `rune sign all` verify with `git verify-commit --raw` against `KEYS`, the `rune sign --verify` check. Neither pushes. | none | The failure names the foreign fingerprint. | nonzero on a foreign key |
| The signer locks the repository queue before it signs | `rune sign next`, `rune sign all`. One lock per repository git directory. | none | The second invocation reports the lock holder. | nonzero for the second invocation |
| Claims are recoverable | `rune sign next`, `rune sign all`. A claim records the process id and the start time. The bounded age is `CLAIM_MAX_AGE_SECONDS`, 3600 seconds. | none | A foreign key is recorded as `failed`. | 0 when a reclaimed head is recorded as signed |
| The ceremony command and the queue do not mix | Bare `rune sign`, `rune sign --amend`, `rune sign --tag <name> [commit]`, and `rune sign --verify [ref]` keep their meaning. `rune sign --tag v1 next` tags the commit named `next`. `rune sign next --tag v1` is rejected. | none | The parser names the unexpected flag. | nonzero for a queue subcommand with a ceremony flag after it |
| Requests are inspected and withdrawn | `rune sign show <bookmark>`, `rune sign drop <bookmark>`. `--repo <dir>` names the repository. `--id <request>` selects by request id. | none | The refusal names the missing request. | nonzero when no request exists |
| Nothing the repository configures is trusted | `rune sign next` passes `signing.backends.gpg.program=<the owner's user-scope value>` and `signing.behavior="drop"` on the jj command line. The repository-scope `signing.*` keys are never read. | none | none | none specified |
| Every read goes through git | `jj git export`, `git merge-base`, `git verify-commit --raw` with the gpg program pinned on the command line. | none | The refusal names the conflicted bookmark. | nonzero |
| The signer refuses an aliased template keyword | `jj config get template-aliases.<keyword>` detects an aliased keyword before `rune sign next` or `rune sign all` signs. | none | `the jj config defines template-aliases.<keyword>, which changes what jj reports: unset it before signing` | nonzero |
| The repository lock is an operating-system file lock | `rune sign submit`, `rune sign drop`, and `rune sign queue --prune` take the same lock. `rune sign queue <bookmark> --json` prints one JSON (JavaScript Object Notation) object. | none | none | none specified |

The decision record is [CLI-0041 Signing Queue](../../decisions/CLI-0041%20Signing%20Queue.md).
