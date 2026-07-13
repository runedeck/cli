---
title: "Watchlist Monitored-Source Registry"
description: "rune watch maintains a user-curated registry of module and deployment locations to monitor, as local paths or SHA-pinned remotes"
type: adr
category: cli
tags:
    - cli
    - watch
status: accepted
created: 2026-06-04
updated: 2026-06-23
author: "@N4M3Z"
project: rune-cli
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Watchlist Monitored-Source Registry

## Context and Problem Statement

rune acts on the module or deployment at the current directory. A developer usually has other locations they care about too: module sources elsewhere on disk, downstream repos that adopt rune artifacts, or a remote repo pinned to a known commit. rune has no way to keep track of those: no registry of locations to watch, and no way to watch a remote at all. Everything is implicit in wherever a command happens to run.

## Decision Drivers

- An explicit, user-controlled list of locations rune should keep track of, beyond the working directory and home
- The ability to watch a remote repo at a pinned commit, not only local paths
- Reuse of the existing `.rune` security and fetch model rather than a second clone path

## Considered Options

1. **Auto-discover sibling directories.** Zero-config, but implicit: it misses repos elsewhere on disk and gives the user no way to say what they actually want watched.
2. **An explicit `rune watch` registry.** A file the user curates, listing exactly the locations to monitor, local or remote.

## Decision Outcome

`rune watch` maintains a registry at `~/.config/rune/watchlist.yaml`: a `locations` list managed by `rune watch list` / `add` / `git` / `remove`.

An entry is either a local path string or a SHA-pinned remote `{ git: <https-url>, ref: <40-hex-sha> }`, the same shape `.rune` uses. Remotes go through the same validators (HTTPS-only, lowercase 40-hex SHA, no embedded credentials) and the same content-addressed cache fetcher, then resolve to a local worktree like any other path; a fetch failure logs and skips that entry. The file is plain YAML and backward compatible, so a bare list of path strings still parses. Mutations load it strictly: a malformed or unknown-key file is reported and left untouched rather than overwritten.

## Consequences

- The watched set is explicit and user-curated; there is no implicit discovery to reason about.
- Remote watching reuses `.rune`'s validators and cache, so HTTPS and SHA-pinning live in one code path, not two.
- An entry records only its location. Storing anything more per entry later is a schema addition.

## More Information

- `rune watch git` and `.rune` git sources share the same validators and fetch path (`src/cli/dotrune/`).
