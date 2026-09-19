---
adr: "docs/decisions/CLI-0041 Signing Queue.md"
status: proposed
---

# Signing Queue

## Why

A session prepares a candidate head, runs the check stages on it, and then needs the owner's key. Today the handoff is a
script in a scratch directory that lists the workspaces and bookmarks to sign, rewritten by hand for every round. The
script does not know the stack order, cannot tell a head that moved after it was written from one that did not, and two
shells signing the same commit at once leave jj with divergent twins. The owner learns that a signature is needed only
by reading the transcript.

## What Changes

- `rune sign` keeps its meaning, the owner's seal, tag, and verify ceremony, and gains a queue of signing requests as
  subcommands.
- `rune sign queue <bookmark>` and its alias `rune sign submit <bookmark>` record a request from a session: repository,
  workspace, bookmark, the head's change and commit ids, and the check receipt that qualified it. A request for a signed
  head, a head without a passing receipt, or a bookmark that does not exist is refused. A desktop notification tells the
  owner.
- `rune sign queue` lists every request across repositories in stack order, each current, stale, or signed.
- `rune sign next` and `rune sign all` are the owner's side: under a per-repository lock, sign the recorded commit of
  the first current request, or every one, base first, retry only a pinentry timeout, verify the signature against
  `KEYS`, and record the result. Neither pushes.
- `rune sign show <bookmark>` and `rune sign drop <bookmark>` inspect and withdraw a request. `rune sign queue --prune`
  removes stale and signed ones.
- The deck's CommitSigning companion points at the queue instead of the script.

## Capabilities

- signing-queue (new)

## Impact

- `src/cli/sign/`: the request store, the queue commands, and the jj signing driver beside the git seal.
- `src/cli/mod.rs`: `Sign` gains subcommands. The bare command is unchanged.
- `docs/decisions/`: CLI-0041.
- The deck's `VersionControl` skill: one companion edit, in a separate change.
