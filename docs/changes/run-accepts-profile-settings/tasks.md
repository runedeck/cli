# Tasks

## 1. Implementation

- [x] 1.1 `filter_profile_args` replaces `reject_owned_args`, owned lists move to `owned_options(surface)`, `invoke_surface` filters once and prints the warnings
- [x] 1.2 `codex_config_key`: per-key filter for `-c`/`--config`
- [x] 1.3 `claude_settings_key`: per-key filter for `--settings`
- [x] 1.4 `run` help text names the drop-with-warning behavior
- [x] 1.5 CHANGELOG entry under Unreleased

## 2. Verification

- [x] 2.1 Tests: whole-flag drop, value-taking drop, codex config merge, claude settings merge, empty settings dropped, unowned pass-through, bypass table asserts the drop for every surface, attached short and `=` forms, per-surface arity, neighbor not swallowed, nested codex keys, settings path refused, control characters escaped
- [x] 2.2 Live: `rune run astra@codex` warns on the model key and proceeds to the tool (the tool itself cannot start inside the sandbox)
- [x] 2.3 Recorded proof under `docs/proofs/run-accepts-profile-settings/` with a fake tool binary that echoes its argv
- [x] 2.4 `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `rune spec validate run-accepts-profile-settings`
- [x] 2.5 Adversarial review by astra and grok (workshop `docs/specs/2026-09-22-run-profile-settings/`), eight fixes and three rejections recorded

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
