---
adr: docs/changes/sign-open-branch-tree/adr.md
status: proposed
decisions: ["Sign open reads the branch, and rune-docs tests run in the workspace"]
---

# Sign open branch tree

## Why

Tonight's landings hit four rough edges. `rune sign open` refused a head the owner had signed with `jj sign` first, although the open-seal is a new commit above the head and the signature does no harm. It read `pull-request.md` from the working copy, so a workspace on another branch had to switch before it could open. `rune spec archive` wrote `TBD` as the purpose of every new capability although the proposal names one. `rune spec doctor` reported `no recognized requirements` for a specification whose requirement lacked MUST, hiding the line. And `rune-docs` was a path dependency, not a workspace member, so its 147 unit tests never ran in CI: five of them had been failing since the prose caps landed.

## What Changes

- `rune sign open` accepts an owner-signed head, refuses a signature from a key outside `KEYS`, and reads the body and the body schema from the bookmark's tree. `--repo` is documented.
- `rune sign adopt` judges the outside body against the protected branch's schema.
- `rune spec archive` takes a new capability's purpose from the proposal's `### New Capabilities` bullet.
- `rune spec doctor` names the parse issue and line of a canonical specification it cannot read.
- `rune promote` refuses a draft whose provider copies differ.
- `rune-docs` joins the Cargo workspace, the push hook and CI run `cargo test --workspace`, the five broken unit tests set the name rule off, and the crate passes clippy and rustfmt.

## Capabilities

### New Capabilities

- `sign-open-branch-tree`: `open` seals above an owner-signed head and reads the branch's own body and schema, plus the archive, doctor, promote, and workspace fixes above.

### Modified Capabilities

- `sealed-review-ceremony`: the open refusal list drops the signed head and gains the foreign key, and a new requirement covers the owner-signed head and the branch body.

## Impact

- `src/cli/sign/queue/{open,repo,mod}.rs`, `src/cli/mod.rs`, `src/cli/promote/mod.rs`, `rune-docs/src/spec/{mod,model,doctor,changelog}.rs`, `Cargo.toml`, two workflows, the push hook, `tests/sign_queue.rs`, and the crate's unit tests.
