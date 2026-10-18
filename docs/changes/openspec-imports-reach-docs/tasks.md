# Tasks

## 1. Implementation

- [x] 1.1 Autodetected `openspec/` root redirects the import destination to `docs`
- [x] 1.2 `.workspaces`, `.worktrees`, `.codex-prs` in the adoption doctor's skip list
- [x] 1.3 `transport_can_authenticate` and `helper_applies` in `repo.rs`, the preflight in `qualify`, `gh auth git-credential` on the push, URL-scoped helpers in `trusted_credential_helpers`
- [x] 1.4 `verify-seal` reads `pull_request`, falls back to `number`, in `scripts/` and the embedded skeleton copy

## 2. Verification

- [x] 2.1 `import_from_an_autodetected_openspec_root_lands_in_docs` and `doctor_skips_sibling_checkouts_under_the_module`
- [x] 2.2 `open_refuses_an_https_origin_with_only_a_repository_scope_helper` fails on `981adc82` and passes here
- [x] 2.3 `cargo test --workspace --all-features` (1790), clippy, rustfmt
- [x] 2.3 Both prek stages in an isolated clone

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
