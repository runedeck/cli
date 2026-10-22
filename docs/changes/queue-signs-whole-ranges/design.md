# Design for queue signs whole ranges

## Context

The [proposal](proposal.md) states the scope and the [draft decision record](adr.md) gives the reasons. The
[delta specification](specs/queue-signs-whole-ranges/spec.md) states the behavior in role terms. This file maps each
requirement to the command, flags, and strings that implement it.

## Commands

| Requirement | Command and flags | Diagnostic ids and messages | Exit status |
|---|---|---|---|
| A session queues a range for one signing run | `rune sign queue <bookmark> --range --receipt <file> [--repo <dir>] [--json]`. The base is `refs/remotes/origin/<main\|master\|trunk>`. The range is `git rev-list --reverse --topo-order <base>..<head>`. The publish refusal is the head request's `refuse_published_head`. | `sign.range_empty`: `<bookmark> at <head> adds no commit to <protected ref>: there is nothing to sign`. `sign.head_published` as for a head request. `--json` adds `"kind": "range"` and `"commits": <n>`. | 0 queued, nonzero refused and nothing recorded |
| The owner sees every commit before the key | `rune sign next`, `rune sign all`, `rune sign show <bookmark>`. The answer comes from `/dev/tty`, or the file `RUNE_SIGN_TTY` names in tests. | The view has `range <base>..<head>`, `commits <n>`, one `author <name> <n>` line per author, and one `<short id>  <author>  <first line>` line per commit. The prompt is `sign <n> commits on <bookmark> from <base> to <head>? [y/N]`. A decline reports `declined`. | nonzero after a decline, the request stays current |
| The signer signs and checks the whole range | One `jj sign -r 'commit_id("<id>") \| ...'` with the head request's pinned `--config` overrides, then `git verify-commit --raw` for every commit in `git rev-list <base>..<new head>`. | `signed <bookmark> <new head>`. Failure reasons: `<n> commits between <base> and <head>, the request recorded <m>`, `<short id> changed identity while signing`, `<short id>: signature from key <fingerprint> is not in KEYS`, `<short id>: signature rejected: <status>`. | 0 signed, nonzero failed |
| A range request goes stale when its range changes | `derive` compares the recorded identities with `git rev-list <base>..<head>` and the protected ref with the base. The signer runs the same check again after `owner_keys` refreshes the protected ref. | `skipped: stale`. After the refresh: `stale: <protected ref> moved to <short id> since the request: rebase and queue again`. | nonzero |
| The signer names each commit in a form that nothing can redirect | `Repo::sign` builds `commit_id("<id>")` for every commit, for head, seal, and range. The guard reads `jj config list revset-aliases -T <listing>`. | `sign.alias_shadows_commit_id`: `the jj config defines revset alias <name>, which redefines commit_id(): unset it before signing` | nonzero, nothing signed |
| A signed request whose head moved says so | `rune sign queue`, `next`, `all`, `queue --prune`. | List state `moved`. Signer line: `skipped: moved: signed at <short id>, the bookmark moved since`. The run ends with `prune signed requests whose bookmark moved with rune sign queue --prune`. | unchanged by a moved request |

## Store

`Request` gains kind `range` and `range: { protected, base, commits: [Identity] }`. `protected` is the remote-tracking
ref name, `base` its commit id at request time, and `commits` the identities oldest first. `head` holds the last entry,
so every field that reads `head` keeps working. A record without `range` reads as before.

## Risks

- A range of many commits makes a long view. The view prints one line for each commit and no diff, so it stays
  readable at 77 commits.
- `jj sign` may rewrite descendants outside the range. `signing.behavior` is pinned to `drop`, so they lose any
  signature. A descendant that the queue holds keeps its identity and stays current.
- The alias guard parses jj's listing. The listing template uses only the `name` and `value` keywords, which
  `user_setting` already checks against `template-aliases`.
