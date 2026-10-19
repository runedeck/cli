---
title: "The consumer manifest may live inside a .rune directory"
description: "rune reads the consumer manifest from the file .rune or from .rune/manifest.yaml under a .rune directory, so the manifest and the records home .rune/records share the directory; a directory without a manifest is no consumer"
type: adr
category: architecture
tags:
    - cli
    - manifest
    - records
status: proposed
created: 2026-10-19
updated: 2026-10-19
author: "@N4M3Z"
project: cli
related:
    - "CLI-0024 Interactive and Automated Tool Commands"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["claude-fable-5-1"]
informed: []
upstream: []
change: manifest-inside-rune-directory
---

# The consumer manifest may live inside a .rune directory

## Context and Problem Statement

On 2026-10-19 the owner decided that every model call leaves a record under `.rune/records/` in the repository, or under `~/.rune/records/` with `--global`, and declined every other home. The consumer manifest is the file `.rune` at that path. A repository cannot hold a file and a directory of one name, and rune refused a `.rune` directory as an error, so the first council run in runedeck/mods ended every seat on ".rune must be a file, not a directory".

## Considered Options

1. The manifest moves inside: `.rune/manifest.yaml` when `.rune` is a directory, the file `.rune` otherwise, read and written through one helper.
2. The records move out, to a home beside `.rune`. The owner declined other homes.
3. A second manifest name beside `.rune`, such as `rune.yaml`. Two names for one file, and every reader learns both for good.

## Decision Outcome

Option 1. Every reader and writer MUST ask `rune::consumer::manifest_path` where the manifest is. A `.rune` directory without `manifest.yaml` MUST read as no manifest. A writer MUST keep the form that is there, and MUST write the file form when neither exists.

## Consequences

- A repository that only records model calls is no consumer, and `rune context` reads it as plain.
- A migration is a move of one file into the directory. Nothing migrates by itself, and the file form stays the default for a new manifest.
- Tools outside rune that read `.rune` as a file see a directory after a migration.
