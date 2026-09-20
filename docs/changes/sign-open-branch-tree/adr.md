---
title: "Sign open reads the branch, and rune-docs tests run in the workspace"
description: "rune sign open seals above an owner-signed head and reads the body from the bookmark's tree, and rune-docs joins the Cargo workspace so its unit tests run in CI."
type: adr
category: process
tags:
    - cli
    - signing
    - testing
status: proposed
created: 2026-09-20
updated: 2026-09-20
author: "@N4M3Z"
project: cli
related:
    - "CLI-0042 Sealed Review Ceremony Commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: sign-open-branch-tree
---

# Sign open reads the branch, and rune-docs tests run in the workspace

## Context and Problem Statement

`rune sign open` was written for a session branch that is unsigned and checked out. Tonight the owner signed heads with `jj sign` before `open`, and `open` ran from a workspace on another branch: both were refused. Separately, `rune-docs` was a path dependency only, so `cargo test` never compiled its unit tests, and five had been failing since the prose caps landed.

## Considered Options

1. Keep the signed-head refusal and tell the owner to run `open` before `jj sign`.
2. Accept a head whose signature verifies against `KEYS` and refuse any other signature.
3. Read the body from the working copy and require `open` to run on the branch.
4. Read the body and its schema from the bookmark's own tree.
5. Leave `rune-docs` out of the workspace and run its tests by hand.
6. Make it a workspace member and run `cargo test --workspace` everywhere.

## Decision Outcome

Options 2, 4, and 6. The open-seal is a new commit above the head, so an owner signature beneath it is harmless and refusing it only forces an order the owner does not want. The branch's tree is what the draft carries, so the body must come from there. A test that never runs is not a test.

The tooling MUST keep these rules:

- `open` MUST refuse a head signature that does not verify against `KEYS`.
- `adopt` MUST judge an outside body against the protected branch's schema, never the outside tree's.
- The push hook and CI MUST run the workspace's tests, not the root package's alone.

## Consequences

- `open` works from any workspace of the repository, and after a `jj sign`.
- The `rune-docs` unit tests turn the name rule off through `set_name_rule_lookup`, because their fixtures use one-word names. The CLI's integration tests cover the rule.
- Clippy's `too_many_lines` split `changelog::lint` into three helpers.
