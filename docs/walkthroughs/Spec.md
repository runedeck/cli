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

## Review checklist

- [ ] Native `docs/`, direct `openspec/`, and a custom root complete the lifecycle.
- [ ] Nested capabilities validate, show, and archive at the matching path.
- [ ] Import and export restore unknown text and binary artifacts byte for byte.
- [ ] `rune spec validate --json` retains every diagnostic field, including `null` values.
- [ ] Completed archive and import retries preserve results without extra writes.

The executable fixtures and recovery checks are in [Manual Testing](../Manual Testing.md#spec).

[OPENSPEC-160]: https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0 "OpenSpec v1.6.0 release"
