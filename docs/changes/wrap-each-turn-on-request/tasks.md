# Tasks

## 1. Wrapper

- [x] 1.1 `src/cli/hook/wrap.rs`: the set in one slot, the lock, the markers, register on `prompt.before`, the wrap on `turn.finish` under the lock, the card with the undo by content, the session key, the workspace root, the legacy script
- [x] 1.2 `src/cli/hook/adapters.rs`: `turn-wrap` as a built-in adapter
- [x] 1.3 Unit tests in `src/cli/hook/wrap/tests.rs`: card text, the shared message, key, slot round trip, a broken and an old slot, markers folded, the lock, open and finish under a held lock, armed by `cwd` or `CLAUDE_PROJECT_DIR`, nothing outside jj or with the switch off

## 2. Dispatcher

- [x] 2.1 `src/hooks.rs`: the wrapper's default timeout, the wrapper last on `turn.finish` whatever its order
- [x] 2.2 `src/cli/hook/run.rs`: `RUNE_MERGED_EFFECT` for every handler, the hold on `turn.finish` only where the switch is on, capped at half the budget
- [x] 2.3 `src/cli/hook/cover.rs`: the owner's `turn/wrap` script as the `turn-wrap` adapter's command
- [x] 2.4 Tests in `src/cli/hook/dispatch_tests.rs` and `install.rs`: order, merged effect, the hold with the switch on and none without, default and explicit timeout, the legacy part reported as not first

## 3. Switch

- [x] 3.1 `src/cli/mod.rs`, `src/cli/surface/isolation.rs`: `rune hook wrap on|off|status|reset` with `--json`, structured errors outside a jj workspace and inside a model session
- [x] 3.2 `templates/skill/SKILL.md`: the owner's switch and reset, that a model never runs them, and who sees the card
- [x] 3.3 `tests/hook_wrap.rs`: switch round trip, refusal outside jj and inside a session, reset, one turn wrapped, a described working copy, two sessions, a prompt during a held commit, a held lock, a malformed slot, a continued turn
- [x] 3.4 `tests/fixtures/hooks/turn-wrap/`: the wrapper's config for proofs

## 4. Review fixes

- [x] 4.1 `src/cli/hook/wrap.rs`: register on every event that shows a session in the workspace, the stop included and a tool event by its written path, the `unknown` key for a payload without a session id, the key written before the first snapshot, no wrap after a failed or timed-out handler, the card's conflict note and the described card's undo
- [x] 4.2 `src/cli/hook/run.rs`: `RUNE_FAILED_HANDLERS` for every handler, and `wrap_hold` only when this session is alone and no legacy part covers the wrapper. `src/hooks.rs`: the timeout comment
- [x] 4.3 Tests that failed before each fix: `wrap/tests.rs` (tool event registers, unknown session, alone, failed handler, key before snapshot), `dispatch_tests.rs` (failed handlers, no hold with another session on record), `install.rs` (no hold under a legacy part), `tests/hook_wrap.rs` (write from another directory, stop without a session id, failed stop handler, first snapshot past the timeout)
- [x] 4.4 Spec, design, and the skill template's turn-wrap paragraph updated

## 5. Review fixes, second round

- [x] 5.1 `src/cli/hook/wrap.rs`, `src/cli/hook/adapters.rs`: a shell command registers the session in every armed workspace it names by a path, and `written_paths` reads `notebook_path`
- [x] 5.2 `src/hooks.rs`: `TURN_WRAP_EVENTS` and `Handler::subscriptions`, so the wrapper hears all four events whatever its config declares. `proposal.md` and `adr.md` name the four
- [x] 5.3 `src/cli/hook/run.rs`, `src/cli/hook/wrap.rs`: `hold` and `CARD_HOLD`, so a shared workspace still reserves the time the card needs
- [x] 5.4 `src/cli/hook/wrap.rs`: the slot and the markers keep each key's harness and first touch, and the card names a session by those
- [x] 5.5 Tests that failed before each fix: `wrap/tests.rs` (a shell command registers, the paths a command names), `hooks.rs` (the four subscriptions), `dispatch_tests.rs` (the card hold), `tests/hook_wrap.rs` (a shell command from another directory, every touch heard with one event declared, a slow stop handler in a shared workspace, the card without a key)
- [x] 5.6 Spec, design, proposal, and ADR updated, with the unregistered shell edit as a limit

## 6. Verification

- [x] 6.1 `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo test --all-features`
- [ ] 6.2 `rune spec validate wrap-each-turn-on-request`, `prek run --all-files`
- [ ] 6.3 Proofs recorded on the final stack
- [ ] 6.4 CHANGELOG entries, archive moves `adr.md` to `docs/decisions/`
