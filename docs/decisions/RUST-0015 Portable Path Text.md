---
title: "Portable Path Text"
description: "Every path that becomes text is written with forward slashes, so manifests, snapshots, and provenance read the same on Windows, macOS, and Linux"
type: adr
category: rust
tags:
    - rust
    - windows
    - paths
    - manifest
status: proposed
created: 2026-11-15
updated: 2026-11-15
author: "@LangrMartin"
project: rune-cli
related:
    - "RUST-0003 Code Style and Tooling"
    - "RUST-0010 Dependency and Lint Configuration"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Portable Path Text

## Context and Problem Statement

Rune stores and compares relative paths as text. Manifest keys, provenance paths, source snapshot entries, bundle entries, and the keys that `doctor` and `drift` compare are all strings, and the code splits and matches them on `/` (`strip_prefix("rules/")`, `split_once('/')`).

On Windows a `Path` turned into text carries `\`. Two things then go wrong:

- Matching fails. A rule at `rules\Name.md` is not recognized as a rule, so it deploys under `rules/rules/` and the manifest gets backslash keys.
- Validation refuses the path. `source_snapshot` and `bundle` reject any text that contains `\` as an ambiguous separator. On macOS and Linux that check catches a suspicious file name. On Windows it rejects every nested path and the deck root itself, so `rune install` stops with `source path has an ambiguous separator or control character`.

The same build had two blockers before any of this ran: `rune-docs` read the Windows file identity through the unstable `windows_by_handle` feature, and a `[target.'cfg(unix)'.dependencies]` header in `Cargo.toml` sat above dependencies that every platform needs.

## Considered Options

1. **Convert at each call site**: add `.replace('\\', "/")` wherever a failure shows up, as `assemble/sources.rs` and `graph/mod.rs` already do in one place each.
2. **One helper at the boundary**: every relative path that becomes text passes through `services::paths::slash_text` or `slash_separators`.
3. **Never compare paths as text**: carry `PathBuf` and components through the manifest, snapshot, and provenance code.
4. **Declare Windows unsupported**: keep the Unix-only assumption and say so.

## Decision Outcome

Option 2. The text form of a relative path is `/`-separated on every platform.

- Convert where a `Path` becomes a `String` that is stored, compared, or split. Use `rune::services::paths::slash_text` for a `Path` and `slash_separators` for text that came from `to_str()`. Do not call `to_string_lossy()` directly on such a path.
- On Windows the helper rewrites `\` to `/`. No Windows file name can contain `\`, so the rewrite loses nothing. On other platforms the helper returns the text unchanged.
- The ambiguous separator check runs after the conversion. It keeps rejecting a literal backslash in a name on macOS and Linux and no longer rejects ordinary Windows paths.
- Text read back from disk gets the same conversion. `manifest::read` converts keys and provenance paths, so a manifest written with `\` keys by an earlier deploy tool names the same files and its entries are not pruned as stale.
- Slash text may be joined back onto a `Path`. `Path::join` accepts `/` on Windows.
- An absolute path that is only shown to the user keeps the native form.

Option 1 leaves the next unconverted site to fail in the field. Option 3 is the larger and safer design, and it can follow later without changing the stored format this record fixes. Option 4 contradicts the Windows entry that the release workflow already carries.

### Consequences

A manifest written on Windows has the same keys as one written on macOS or Linux for the same content, so a deployment can be compared across machines.

File identity on Windows comes from the open handle through `winapi-util`, a safe wrapper, because stable `std` does not expose the volume serial number and file index. `unsafe_code = "forbid"` stays in force.

Directory syncing in the spec transaction is skipped on Windows, where `File::open` cannot open a directory.

Still open on Windows:

- Symlink assembly and deployment return an explicit error.
- Integration tests isolate the home directory through `HOME`. `dirs::home_dir` ignores `HOME` on Windows, so those tests would write to the real home directory and must not run there until rune has a home override.
- `target::Scope::Project` derives its key by replacing `/` only.
- Three library tests assert on the native text of an absolute path and fail on Windows.

## More Information

Verified on Windows 11 with the `stable-x86_64-pc-windows-gnu` toolchain: `rune install`, a repeated install, `rune doctor`, `rune drift`, and `rune provenance` against the runedeck deck in an isolated target; the `rune-docs` test suite; the library unit tests apart from the three named above.
