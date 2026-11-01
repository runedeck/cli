## 1. Configuration

- [x] 1.1 The `ask:` section: `model`, `questions`, and `routes.<name>` with `id`, `base_url`, and `key_env`, on `Config` and `ResolvedConfig`
- [x] 1.2 `key_env` and `questions` leave `launch.tools.<name>`, with their recovery alias
- [x] 1.3 The configuration reference lists the `ask` keys and no longer the removed ones

## 2. Command

- [x] 2.1 `rune ask <QUESTIONS> [TEXT]` with `--file`, `--state-file`, `--model`, `--timeout`, `--dry-run`, `--check`, and `--json`, in the root help under Plumbing
- [x] 2.2 Route resolution: the built-in `jev`, a configured override of it, any configured route, and a model id on the default route
- [x] 2.3 Question-set lookup by path or by name in the configured folders, sending `questions` alone
- [x] 2.4 State from the text argument, the text file, stdin, or the state file
- [x] 2.5 The key read at call time from the environment or the env file, joined to the redaction set, and omitted for a route without one
- [x] 2.6 The request, the reply, the text form, the JSON form, the exit codes, the dry run, and the check

## 3. Removal

- [x] 3.1 `jev` out of the `rune run` and `rune launch` tool lists, `--questions` and `--state-file` out of `rune run`, the model-tool help section gone
- [x] 3.2 `rune run jev` and `rune launch jev` refused with one line that points at `rune ask`

## 4. Documentation

- [x] 4.1 README, CHANGELOG, and the example configuration

## 5. Verification

- [x] 5.1 Unit tests in `src/cli/ask/tests.rs`, `src/cli/launch/tests.rs`, and `src/ontology/tests.rs`
- [x] 5.2 Integration tests in `tests/ask.rs`: the refusals, `--file` and stdin, a route other than `jev`, and the config keys
- [ ] 5.3 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `rune spec validate ask-questions-about-text`
- [ ] 5.4 Live: `rune ask --check`, then one hosted request with a question set from the owner's folder and a synthetic text
