# Spec Walkthrough

`rune spec` manages specification changes in Rust. Normal lifecycle commands read, validate, and write artifacts directly without invoking the upstream OpenSpec CLI.

## What it manages

```text
repository
├── docs/                       native root
│   ├── changes/<change>/
│   │   ├── proposal.md
│   │   ├── design.md           optional
│   │   ├── tasks.md
│   │   └── specs/<capability>/spec.md
│   └── specs/<capability>/spec.md
├── openspec/                   OpenSpec root or conversion target
└── <custom root>/              spec.root override
```

The parser and delta application behavior are compatible with [OpenSpec v1.6.0][OPENSPEC-160] at the artifact boundary. `docs/`, `openspec/`, and custom roots contain the same lifecycle artifacts. Nested capabilities such as `payments/card` retain their path segments.

## Lifecycle

```sh
rune spec propose add-widget --capability widgets --design
rune spec list --sort progress
rune spec show add-widget
rune spec context add-widget
rune spec validate add-widget
rune spec doctor
rune spec archive add-widget
rune spec list --specs
rune spec show widgets
```

- `propose` creates the proposal, checklist, capability deltas, and optional design.
- `show` and `context` include every delta and the design when present.
- `validate [NAME]` checks the tree, one active change, or one canonical capability. `--json` returns stable diagnostics with explicit `null` fields.
- `archive` applies `RENAMED`, `REMOVED`, `MODIFIED`, then `ADDED`, and moves the change into the dated archive.
- Repeating a completed archive succeeds without rewriting unchanged files.

## Root selection

With no override, rune uses the only live tree among `docs/` and `openspec/`. If both contain changes or specifications, set `spec.root` explicitly. A custom root is repository-relative.

```yaml
spec:
    root: docs/openspec
```

The example stores artifacts under `docs/openspec/changes` and `docs/openspec/specs`. Paths that escape the repository and symlinked root boundaries fail before mutation.

On the first interactive command in an OpenSpec-rooted repository without a configured root, rune offers to keep `openspec/` or migrate to `docs/`. Automated and JSON invocations keep the autodetected root, write no configuration, and print one advisory note.

## OpenSpec conversion

```sh
rune spec import --openspec
rune spec validate
rune spec export --openspec
```

Import moves direct change and specification artifacts into the selected root. Other regular files remain opaque and byte-preserved under `.interop/openspec/files/`. The ownership manifest at `.interop/openspec/manifest.yaml` records each original path, classification, and digest. Export verifies that manifest before restoring the complete `openspec/` tree.

Archive, import, and export share a repository lock and transaction journal. The next of these commands recovers interrupted writes, archive moves, and source removals before starting new work. Repeating a completed archive or import preserves unchanged files and their modification times.

## Doctor advisory

On an OpenSpec root, `rune spec doctor` also attempts the optional upstream command:

```sh
openspec validate --all --no-interactive
```

A missing executable produces no finding. A failure or timeout is advisory and does not change the exit code unless rune reports its own structural error.

## House rules

`rune spec validate` and `rune spec doctor` apply three rules beyond the OpenSpec parser, which still accepts `SHALL` and `MUST`:

- Every normative statement uses `MUST`. A prose line with `SHALL` fails with `spec-shall-keyword`.
- A canonical specification stays under 150 lines, or it splits into capabilities. A delta over the limit warns.
- A defined term is a link to its glossary page: `*[term][TAG]*` at the first use in a file, `[term][TAG]` after it, and `[TAG]: <relative page path>` at the end of the file. The validator resolves the link through the page's `iri` and matches it to a term in `ontology/rune.ttl` (or the file `deck.yaml` names under `ontology`). Plain italics are emphasis. Without an ontology there are no terms.
- A link that names no term fails (`spec-term-undefined`), a draft term warns (`spec-term-draft`), a first use that is not emphasized or a later use that is fails (`spec-term-first-use`), a missing or stale page fails (`spec-term-page`), and the old `*term* [TAG]` form with a definition that names the term address warns with the new form (`spec-term-legacy`).
- `rune spec glossary` prints every term with its tag and page definition. `rune spec glossary --write` writes one page per term and an index under `docs/specs/glossary/`, so a term link reaches a page a reader can open. `--check` regenerates every page in memory, names each page that is missing, extra, or changed, and exits 1 on drift or a missing directory. Both take an optional PATH relative to the source root.
- `rune spec fix --terms` adds a draft entry to the ontology for every link that names no term and writes its page. `rune spec fix --links` rewrites the old form to links in every Markdown file. Without a flag it runs both. `rune spec archive` refuses a change whose files link to a draft term or to no term.

```sh
rune spec validate            # house rules run with the structural checks
rune spec doctor              # the same findings beside relationship health
rune spec glossary            # every term, its tag, and the page definition to paste
rune spec glossary --write    # write docs/specs/glossary/ from the ontology
rune spec glossary --check    # exit 1 when a page drifted from the ontology
rune spec fix                 # draft the unknown terms and convert the old form
```

A consumer has no `deck.yaml`, so it reads the deck's ontology through a deployed copy. Name the source under `ontology` in `.rune`:

```yaml
version: 1
sources:
    deck:
        git: https://github.com/runedeck/deck
        ref: <40-hex commit>
ontology: deck
```

`rune install` copies `<source>/ontology/rune.ttl`, or the path the source's `deck.yaml` names under `ontology`, to `ontology/rune.ttl` at the consumer root, with its fingerprint in `ontology/.manifest` and its sidecar at `ontology/.provenance/rune.ttl.yaml`. `rune drift` and `rune status` report a copy that left that record, and a hand-edited copy is skipped on the next install unless `--force` is passed. A source with no ontology file fails the install before any provider tree is written. After the install, run `rune spec glossary --write` and commit the pages, or every term link fails with the page diagnostic.

## Review checklist

- [ ] Native `docs/`, direct `openspec/`, and a custom root complete the lifecycle.
- [ ] Nested capabilities validate, show, and archive at the matching path.
- [ ] Import and export restore unknown text and binary artifacts byte for byte.
- [ ] `rune spec validate --json` retains every diagnostic field, including `null` values.
- [ ] Completed archive and import retries preserve results without extra writes.
- [ ] A `SHALL` line, a 151-line specification, and a term link that names no term each fail validation with the named rule.

The executable fixtures and recovery checks are in [Manual Testing](../Manual Testing.md#spec).

[OPENSPEC-160]: https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0 "OpenSpec v1.6.0 release"
