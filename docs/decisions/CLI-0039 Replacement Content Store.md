---
title: "Replacement Content Store"
description: "An adapt verdict carries the approved text, finalize proves the text is in the file, and the text persists under .provenance/replacements/<sha256> beside the sidecar"
type: adr
category: cli
tags:
    - cli
    - adopt
    - review
    - provenance
status: proposed
created: 2026-09-13
updated: 2026-09-13
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0023 Adoption Review State Machine"
    - "CLI-0027 Temporary Adoption Session State"
    - "ASSEMBLY-0012 Adoption Metadata in Provenance Sidecars"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
---

# Replacement Content Store

## Context and Problem Statement

An `adapt` verdict recorded a rationale note and nothing else. Finalize could prove that the
upstream block was gone, but not that the wording the maintainer approved had arrived: any
edit that removed the block passed (cli#16). The approved text is review evidence. It has to
outlive the temporary session, and the sidecar has to point at it.

## Decision Drivers

- Finalize proves the approved text is in the edited file
- The text persists with the sidecar, not in the session that finalize deletes
- The sidecar stays small
- Existing sessions keep working through a deprecation window

## Considered Options

1. **Session-only verification**: verify at finalize, then discard the text.
2. **Digest only**: record the replacement's sha256 in the sidecar and nothing else.
3. **Content-addressed store**: write the text to `.provenance/replacements/<sha256>` beside
   the adapted file and reference it from `metadata.replacements`.

For input: a file only, a file or inline text, or those plus standard input. For the
transition: required now, optional with a warning, or a deprecation window.

## Decision Outcome

Option 3, with file or inline input and a deprecation window.

`rune adopt verdict <id> adapt` accepts `--replacement <text>` or `--replacement-file
<path>`. The two exclude each other, and any other verdict refuses them. An adapt without a
replacement warns in this release and fails in the next.

Finalize segments the replacement the way its file is segmented and requires every
normalized block in the edited file, beyond the copies kept blocks account for. Replacement
blocks reconcile as adapt results, so they never appear as anonymous `added` entries. The
store is written before the sidecars, so a crash keeps finalize restartable. Each sidecar
lists `block`, `sha256`, and the store path relative to its `.provenance/` directory.

## Consequences

- `.provenance/replacements/` is tracked source. Assembly and deployment never read it,
  because both walk sidecars by file name.
- The store duplicates text that also sits in the artifact. The cost is one small file per
  adapted block.
- A replacement that spans several blocks is one store file and several finalize checks.
- Sessions opened before this change carry no replacement and finalize with the warning.
