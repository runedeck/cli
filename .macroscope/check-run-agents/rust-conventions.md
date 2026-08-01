---
title: Rust Conventions
model: claude-opus-5
input: full_diff
tools:
    - browse_code
    - git_tools
    - github_api_read_only
include:
    - "**/*.rs"
    - "Cargo.toml"
conclusion: failure
showToolCalls: true
---

# Rust review

Review changed Rust code for the conventions below. Flag violations in
touched code only; do not demand unrelated repository-wide cleanup.

## Errors

- Fallible functions return Result with the repository Error struct and
  its non_exhaustive ErrorKind enum (RUST-0001, RUST-0009); no anyhow,
  no thiserror, no bare Result<T, String>.
- Library code never panics: .unwrap() and .expect() are test-only.
  Only binary entry points (main.rs, CLI dispatch) may panic on
  unrecoverable errors.
- I/O errors are never silently erased: no .unwrap_or_default() or
  .ok() swallowing on file reads, network calls, or deserialization.
  Propagate the error or log it before falling back.

## Data

- YAML, JSON, and TOML deserialize into typed structs. Flag chained
  .get() traversal over untyped value objects; schema mismatches must
  fail at parse time, not return silent defaults.
- Paths validated against an allowed directory are canonicalized first
  (std::fs::canonicalize); a raw path with .. components bypasses
  starts_with checks, and falling back to the unresolved path for the
  security check is a violation.

## Style

- Unsafe code stays forbidden through the workspace lints table
  (unsafe_code = "forbid" under [lints.rust] in Cargo.toml); flag any
  edit that weakens or removes it.
- Names carry the documentation: flag comments that narrate what the
  next line does, and abbreviations under five characters where a full
  word fits (manifest, not mfst).
- Multi-line test fixtures live in external files loaded with
  include_str!, never inline strings; generated file content is
  embedded as a template and substituted, not built with format!
  concatenation.
- Blocks nested past three indentation levels extract into a named
  method.
