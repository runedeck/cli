# Tasks

## 1. Implementation

- [x] 1.1 `Surface::Cursor` with its name, boolean table, owned table, and dispatch in `surface/mod.rs`
- [x] 1.2 `surface/cursor.rs`: argument builder, standard input prompt, JSON parser, clean-state rule, sandbox hint
- [x] 1.3 `prepare_clean_state` and `process_request` match cases for a clean Cursor run
- [x] 1.4 Launch: `cursor` in `KNOWN_TOOLS` and `default_binary` shared by `resolve_tool` and `list_tools`
- [x] 1.5 `run` supported-tool message and help text name Cursor
- [x] 1.6 CHANGELOG entry under Unreleased
- [x] 1.7 Built-in `fable@cursor` profile and `fable-cursor` route, listed by `rune launch` and named in the missing-profile error

## 2. Verification

- [x] 2.1 Tests: read-only and workspace-write argument lists, model flag, standard input prompt, JSON success, `is_error`, empty result, invalid JSON, clean run without a key, clean environment with a key, sandbox hint on and off, owned flags dropped with warnings, launch default binary and override
- [x] 2.2 Live, 2026-10-01 outside the sandbox: `rune run fable@cursor` returned `pong`; a read-only run asked to create `hello.txt` wrote nothing under `approvalMode` `auto-review` and under a copied `unrestricted` config; a workspace-write run created the file
- [x] 2.3 Recorded proof under `docs/proofs/cursor-agent-surface/` with a fake tool binary that echoes its argv
- [x] 2.4 `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `rune spec validate cursor-agent-surface`
- [x] 2.5 Review by Fable and adversarial review by astra and grok, with each finding fixed or rejected in writing (rejected: zero output tokens stay `null`, as for codex and grok)

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
