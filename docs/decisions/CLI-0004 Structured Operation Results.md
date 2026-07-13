---
title: "Structured Operation Results"
description: "Typed result structs for all CLI operations with human and JSON output modes"
type: adr
category: cli
tags:
    - cli
    - ux
status: accepted
created: 2026-03-20
updated: 2026-07-13
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0003 Conflict Resolution on Install"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Structured Operation Results

## Context and Problem Statement

CLI operations (install, assemble, validate, copy) touch multiple files across multiple providers. Every operation needs to report what happened — what succeeded, what was skipped, and what failed — in a structured way that's both human-readable and machine-parseable. This applies everywhere, not just install.

## Considered Options

1. **Exit codes only** — 0 for success, non-zero for failure. No detail on what happened.
2. **Structured result types** — typed structs with per-provider breakdown and optional JSON output.

## Decision Outcome

Every operation returns a structured result, not just an exit code:

```rust
pub struct ActionResult {
    pub installed: Vec<DeployedFile>,
    pub skipped: Vec<SkippedFile>,
    pub pruned: Vec<PrunedFile>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub struct DeployedFile {
    pub source: String,
    pub target: String,
    pub provider: String,
}

pub struct SkippedFile {
    pub target: String,
    pub provider: String,
    pub reason: SkipReason,
}

pub enum SkipReason {
    UserModified,
    TargetMismatch,
    Unchanged,
}
```

CLI output:

```sh
rune validate --source .
# validation
#   ✓ module.yaml
#   ✓ agents/Developer.md
#
# ✓ 2 checked  ⚡ 0 warnings  ✗ 0 errors
```

Human output uses compact, colored per-item status lines and an operation-specific
summary. Install and deployment operations group items by provider. Validation
prints one line per checked artifact or external check, followed by checked,
warning, and error counts.

`--json` serializes only `ActionResult` for machine consumption. Human
presentation text and external-check output never precede or follow the JSON
document. An operation exits with status 1 when `ActionResult.errors` is
non-empty, status 0 otherwise, and status 2 for a fatal setup or I/O error.

## Consequences

- [+] Clear reporting of partial installs
- [+] Machine-parseable output for CI integration
- [+] Per-provider breakdown shows what landed where
- [+] Validation findings remain scannable without sacrificing structured JSON
- [+] Exit status has the same error semantics in human and JSON modes
