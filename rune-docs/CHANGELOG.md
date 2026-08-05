# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-07-25

### Added

- `links`: broken internal links and orphan pages across a repo's `docs/` tree.
- `adr`: `<PREFIX>-<NNNN>` decision records under `docs/decisions/` — scaffold,
  list, supersede with cross-links, regenerate the index.
- `spec` (feature `lifecycle`): the spec-driven change lifecycle — propose,
  list, show, context, doctor, archive with delta merges, tree validation, and
  a crash-safe transaction engine with journal-based recovery.
- `interop` (feature `lifecycle`): ownership-preserving converters between the
  native spec root and OpenSpec's `openspec/` layout.
- `support`, `sheet`, `error`: atomic writes, frontmatter splitting, path
  confinement, terminal styling, and the crate error type.
- Spec root autodetection counts an interrupted transaction journal as a live
  tree, so a root whose directories were already moved away still resolves to
  the directory holding the unfinished work.
- Retried conversions acknowledge work completed by crash recovery instead of
  failing with an empty-tree error; reports carry a `recovered` marker.
- Archive journal validation rejects overlapping canonical destinations.
- Export from a root whose manifest recorded no opaque files no longer fails
  planning removal of the absent mirror directory.
