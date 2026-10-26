---
adr: docs/changes/queue-signs-whole-ranges/adr.md
status: proposed
decisions: ["The queue signs a whole range for an owner direct push"]
---

# Queue signs whole ranges

## Why

A direct push to the protected branch must carry the owner's signature on every commit it adds (skeleton `owner-release-ceremony`, Owner Direct Push). The queue signs one commit for each request, and each request needs its own bookmark and a receipt that names that commit. On 2026-10-22 the cli's local `main` was 77 commits above `origin/main` (`git rev-list --count origin/main..main`): 73 by Claude Fable 5.1 and 4 by Claude Opus 5.5. The owner signed the stack by hand with `jj sign` and the queue's pinned settings copied onto the command line. A hand-run command checks no alias, shows no commit, and verifies nothing against `KEYS`.

Two more defects turned up in the signer. First, it names a commit by its bare id. In jj a bookmark or a tag with that name wins over the id, and a revset alias with that name expands to anything. So a session can make the owner's key sign a commit the owner never saw. Second, a request that was signed and whose head moved later lists as `stale`. On 2026-10-22 `rune sign all` printed `skipped: stale` for ten such requests, and that read as "nothing worked".

## What Changes

- `rune sign queue <bookmark> --range --receipt <file>` records one `range` request for the commits that `git rev-list` lists between `origin/<protected>` and the head, oldest first. The bookmark may be the protected branch.
- `rune sign next` and `rune sign all` show every commit of a range, read `y` from the terminal, and sign the commits in one `jj sign` run. They verify each commit against `KEYS` with git and record the new head. The push line names the bookmark, as for a head request. Nothing pushes.
- `rune sign show` lists the commits of a range request.
- Every `jj sign` the queue runs names its commits as `commit_id("<id>")`, and the signer refuses while a revset alias redefines `commit_id`. This also covers head requests and both seals.
- A signed request whose head moved lists as `moved`. `next` and `all` pass over it, name `rune sign queue --prune`, and keep their exit status.

## Capabilities

### New Capabilities

- `queue-signs-whole-ranges`: one request covers every commit that a direct push adds, the owner sees each commit before the key, the signer names commits in a form that nothing can redirect, and a signed request whose head moved says so.

### Modified Capabilities

None.

## Impact

- `src/cli/sign/queue/{mod,repo,store,ceremony}.rs`, the `--range` flag in `src/cli/mod.rs`, unit tests, and `tests/sign_queue.rs`.
- The request record gains kind `range` and an optional `range` field. Older records read unchanged.
- No change to `open`, `submit`, the seal messages, the verifier, or the message queue under `src/cli/queue/`.
