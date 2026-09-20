Make `rune spec import --openspec` write `docs/` when `openspec/` is the only tree, keep `rune adopt doctor` out of sibling checkouts, and make `rune sign open` check its push transport before the key touch.

## Plan

Two defects found while moving the html-tools and docs spec trees: import converted `openspec/` into itself (its root was autodetected as the source), and the adoption doctor read the sidecars of a jj workspace under `.workspaces/` as this checkout's.

## Changes

- Redirect an autodetected `openspec/` root to `docs` in `import_openspec_with_io`, while a configured `spec.root: openspec` keeps in-place ownership. Add `spec_root_is_configured`.
- Add `.workspaces`, `.worktrees`, `.codex-prs` to the adoption doctor's `SKIP_WALK`.
- Add `docs/changes/openspec-import-destination/` and two unit tests.
- Check the push transport in `rune sign open` before the key touch: push through `gh auth git-credential` for that one process, refuse an HTTPS origin with neither a gh login nor a trusted helper, and count URL-scoped helpers as trusted. `verify-seal` reads `pull_request`, the field the seal writes.

## Testing

- [x] `cargo test --workspace --all-features`: 1790 passed.
- [x] clippy and rustfmt clean.
- [ ] Both prek stages in an isolated clone.

## Release Notes

- Fix `rune spec import --openspec` importing a stray `openspec/` tree into itself, `rune adopt doctor` walking into `.workspaces/`, and `rune sign open` signing a seal it cannot push.
