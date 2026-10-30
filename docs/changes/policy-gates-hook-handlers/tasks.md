# Tasks

## 1. Reader

- [x] 1.1 `src/policy/`: the file, the paths, the layers, the repository file, the session switch file
- [x] 1.2 `src/policy/tests.rs`: the cases of `jev/test/policy.test.ts`, and the repository and session layers

## 2. Dispatcher

- [x] 2.1 `src/hooks.rs`: `policy` on a handler and in the plan, with the name check
- [x] 2.2 `src/cli/hook/run.rs`: the skip before the cover check, the log line, the record, no hold for a refused wrapper
- [x] 2.3 `src/cli/hook/mod.rs`: the policy column in `rune hook list`
- [x] 2.4 Tests in `dispatch_tests.rs`, `hooks.rs`, and `hook/mod.rs`

## 3. Command

- [x] 3.1 `src/cli/policy.rs`, `src/cli/mod.rs`: `rune policy check`, the root help line, `tests/fixtures/root-help.txt`
- [x] 3.2 `tests/hook_policy.rs`: the dispatcher skips a refused handler, runs it where allowed, runs a handler without a key, and the check's lines and exit codes

## 4. Verification

- [ ] 4.1 `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo test --all-features`
- [ ] 4.2 `rune spec validate policy-gates-hook-handlers`, `prek run --all-files`
- [ ] 4.3 CHANGELOG entry, archive moves `adr.md` to `docs/decisions/`
