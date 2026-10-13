# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `spec::lint`: the runedeck house rules as a layer over the compatibility parser. Validate and doctor report a prose line that uses `SHALL` (`spec-shall-keyword`, `delta-shall-keyword`), a canonical specification over 150 lines (`spec-too-long`, and `delta-too-long` as a warning), and an italic defined term with no `- **term**: definition` entry in `<specs>/glossary.md` (`spec-term-undefined`, `delta-term-undefined`). The parser still accepts `SHALL` and `MUST`, and the OpenSpec v1.6.0 oracle fixtures are unchanged.

- `spec::terms`: the term source behind the italic-term rule. An `ontology/rune.ttl` (labeled classes, properties, and SKOS concepts) replaces `glossary.md` when present. The first italic use of a term must cite it (`term-reference-missing`, `term-reference-unresolved`). `glossary` prints the term list, and `lint_documents` applies the reference rules to files outside the specification tree, where plain emphasis is not a term.

- `spec::terms`: `write_glossary` renders the glossary file (`render_glossary_file`: a header, the term list, the definitions) from the ontology into `glossary.md` beside the specifications or a given path. `check_glossary` compares the file with the rendering and reports the terms whose lines are missing, extra, or changed (`GlossaryCheckOutput`, `render_glossary_check`), exit 1 on drift or a missing file. A term without a comment renders with one space before its tag.

### Changed

- The scaffolded delta template and the archive fixtures use MUST.
- `spec::terms` sorts two terms that share a label by kind (the class first) and then by name, so the order does not depend on the order of the ontology file.

### Fixed

- `spec::terms` reads the comment of a term whose closing quote directly precedes the terminating period. The reader dropped that quote, so the literal never closed and the term had no definition.

## [0.1.0] - 2026-07-25

### Added

- `links`: broken internal links and orphan pages across a repo's `docs/` tree.
- `adr`: `<PREFIX>-<NNNN>` decision records under `docs/decisions/`, scaffold,
  list, supersede with cross-links, regenerate the index.
- `spec` (feature `lifecycle`): the spec-driven change lifecycle, propose,
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
  failing with an empty-tree error. Reports carry a `recovered` marker.
- Archive journal validation rejects overlapping canonical destinations.
- Export from a root whose manifest recorded no opaque files no longer fails
  planning removal of the absent mirror directory.
