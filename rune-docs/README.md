# rune-docs

Documentation lifecycle for [rune](https://github.com/runedeck/cli), packaged
as a self-contained library crate: `docs` link checks, architecture decision
records, and spec-driven changes.

The rune CLI consumes this crate behind its `docs`, `adr`, and `spec` cargo
features and works without it; nothing here depends on the CLI. Errors are the
crate's own type, converted at the CLI boundary, and anything the crate cannot
know by itself arrives as an argument or installed hook: the repo's configured
`spec.root` via `spec::set_root_config_lookup`, the mdschema validator as the
`MdschemaCheck` parameter of `spec::validate_spec_tree`.

## Modules

- `links`: broken internal links and orphan pages across a repo's `docs/` tree
- `adr`: `<PREFIX>-<NNNN>` decision records under `docs/decisions/` — scaffold,
  list, supersede with cross-links, regenerate the index
- `spec` (feature `lifecycle`): the spec-driven change lifecycle — propose,
  list, show, doctor, archive with delta merges, plus the read-only scans and
  tree validation
- `interop` (feature `lifecycle`): converters between the native spec root and
  `OpenSpec`'s `openspec/` layout
- `support`, `sheet`, `error`: the crate's own atomic writes, frontmatter
  splitting, path confinement, terminal styling, and error type

Templates (`templates/spec/`) and mdschemas (`schemas/`) ship with the crate;
a repo can override any of them by placing a replacement at its source root.

## Conventions

The CLI repository's decision records bind this crate where they name Rust
conventions: RUST-0004 (fixtures under `tests/fixtures/`, loaded from unit
tests via `include_str!`; the directory holds data, not integration
binaries), RUST-0008 (no traits for internal types; `spec::transaction`'s
`TransactionIo` is the documented failure-injection exception), and
RUST-0012 (separated test files: every module's tests live in a sibling
`tests.rs` declared with `#[cfg(test)] mod tests;`, never inline). New
tests follow the sibling pattern; the `OpenSpec` v1.6.0 oracle suite is
`src/spec/openspec_oracle.rs`. The public surface is pinned by
`tests/public_api.rs`, and both output byte forms are frozen by the golden
fixtures under `tests/fixtures/output/`.

## Building

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

The CLI consumes this crate by path (`rune-docs` from the repository root), so
building the CLI rebuilds this crate in place from the same tree.

## License

EUPL-1.2
