## 1. Recovery and guidance

- [x] 1.1 Extend the error type with `code` and `fix_command` and render errors once
- [x] 1.2 Migrate setup, config, provider, install, and doctor errors to the structured form
- [x] 1.3 Add `rune config check` for the user and source scopes
- [x] 1.4 Add `rune config defaults` with commented output from the installed binary
- [x] 1.5 Add `rune config reference` with compiler-backed key metadata
- [x] 1.6 Publish `docs/agent-guide.md` with read-only first steps

## 2. Provider insight

- [x] 2.1 Build the bundled detection registry beside the embedded provider defaults
- [x] 2.2 Add `rune provider status` with the six lifecycle states
- [x] 2.3 Add `rune provider explain` with evidence, state, and fix command
- [x] 2.4 Move setup, context, status, doctor, and drift to the shared registry
- [x] 2.5 Add the config-reference drift check to CI

## 3. Safe mutation and onboarding

- [ ] 3.1 Add the syntax-preserving config editor with the managed override fallback
- [x] 3.2 Add scoped `rune config reset` with backup, verification, and atomic write
- [ ] 3.3 Protect modified installed skills from silent replacement
- [x] 3.4 Extend `rune setup` with plan, approval, apply, verification, and the versioned record
- [x] 3.5 Add the first-run nudge in the dispatch path, independent of the `tui` feature

## 4. Verification

- [ ] 4.1 Test plan-only mode, apply order, and the versioned setup record
- [ ] 4.2 Test check exit codes, defaults output, reset backup and restore, and reference drift
- [ ] 4.3 Test lifecycle states, explain evidence, and the protected `modified` state
- [ ] 4.4 Run formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and the tests
