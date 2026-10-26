## 1. Implementation

- [x] 1.1 `Surface::Cursor` with its name, boolean table, owned table, answer-only refusal, and dispatch in `surface/mod.rs`
- [x] 1.2 `surface/cursor.rs`: argument builder, standard input prompt, JSON parser, access check, clean-state rule, Keychain hint
- [x] 1.3 `prepare_clean_state` and `process_request` match cases for a clean Cursor run, and `check_access` in `run/mod.rs` before the workspace exists
- [x] 1.4 Launch: `cursor` in `KNOWN_TOOLS`, `default_binary` shared by `resolve_tool` and `list_tools`, `BUILT_IN_PROFILES`, `BUILT_IN_ROUTES`, `model_args`
- [x] 1.5 `run` and `launch` help text name Cursor, and `rune run` strips the generated `--model` pair
- [x] 1.6 README run and launch sections and CHANGELOG entries under Unreleased
- [x] 1.7 Review fixes: `split_option` names a dropped flag without its attached value, `config_dir` joins a relative directory to the repository, and `top_level_records` with `result_object` take the result from the last top-level JSON record only
- [x] 1.8 Redaction redesign: `refuse_url_credentials` refuses a URL with a user or password in any argument, decoded value, route value, or inherited base URL variable, naming the credential source. `child_secrets` and `known_secrets` read only the route's environment. `redact_output` is the one output function for the dry run, the reply, the JSON, and the check report. `is_credential_query_name` splits on camelCase and `redact_one_url` hides every credential-named parameter. The userinfo forms, the WHATWG and Base64 variants, and the user-only path are removed
- [x] 1.9 Parser-decided refusal: `url_userinfo` finds each candidate by its scheme and asks the `url` crate for its user and password, replacing the `://` scan. A password refuses everywhere, a user name only in an endpoint place (`CredentialPlace`), and a proxy variable is skipped. `proxy_credentials` adds each proxy password or token-only user, as written and decoded, to `known_secrets` and `child_secrets`. `known_secrets` reads codex `-c` strings decoded. `redact_argument` decodes a TOML value before the URL pass (`redact_toml_urls`). `split_at_urls` ends a URL at the quote that opened it

## 2. Verification

- [x] 2.1 Adapter tests in `surface/tests.rs`: read-only and workspace-write argument lists, model flag, standard input prompt, JSON success, `is_error`, no result object, clean-state key rule, profile allow-list, approval-mode refusal, Keychain hint, mode table
- [x] 2.2 Launch tests: built-in `fable@cursor`, configured profile and route replacement, default binary, missing-profile error
- [x] 2.3 Run tests with a recording fake: stdin prompt and result object, `fable@cursor` without config, reported error, clean run with and without the key, profile key from the environment, approval-mode refusal, Keychain hint
- [x] 2.4 `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo test --all-features`
- [x] 2.8 Review-fix tests: adapter tests for the short-flag warning forms, the repository-relative config directory, the nested success under a top-level error, and malformed output. Run tests with the fake for the attached short-option secret, the relative `CURSOR_CONFIG_DIR` with `--repo`, and the nested or truncated result
- [x] 2.9 Redaction tests, each failing before the change: unit tests for the refusal on cursor, codex, and claude by name of the credential variable, the camelCase query split over every parameter, `url_query_secrets`, the URL userinfo decision, and `redact_output` on text and JSON. Run tests with the fake for a password with a `'` refused with exit 125 before the harness starts, the dry run refusing a bare-token and a TOML-escaped URL, and `about postgres, anonymous and documentation` printed whole beside inherited `DATABASE_URL`, `FTP_URL`, and `SEARCH_URL`
- [x] 2.10 Parser tests, each failing before 1.9. `url_userinfo` over the backslash, one-slash, three-slash, no-slash, tab, upper-case, quoted-password, prose, and non-special-scheme forms. A user-only `DATABASE_URL` resolving, while a password there, a user-only base URL variable, and a user-only endpoint flag refuse. A proxy variable resolving with its password in the known values and hidden in the dry run. A TOML-escaped codex query secret hidden in `redact_argument`. `child_secrets` holding proxy passwords and token-only users. A run test with the fake: `threeslash@cursor` refused before the harness starts, and the dry run refusing the backslash, one-slash, no-slash `-c`, and inherited tab forms with exit 125 and no host or secret printed
- [ ] 2.5 `rune spec validate cursor-joins-run-and-launch` and prek, by the caller
- [ ] 2.6 Live, outside the sandbox: `rune run fable@cursor "Reply with pong"` answers, a read-only run that asks for a file writes nothing, a workspace-write run creates it
- [ ] 2.7 Recorded proof under `docs/proofs/cursor-joins-run-and-launch/` with a fake tool binary that echoes its argv

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
