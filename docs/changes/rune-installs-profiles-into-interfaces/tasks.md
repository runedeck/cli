## 0. Read-only slice for 0.7.0

- [ ] 0.1 `rune interface list`, `status t3code`, and `enable t3code --dry-run`, with `--json`. `enable` without `--dry-run` refuses
- [ ] 0.2 The planner: profile data only, the native instance for a profile without environment values, an instance beside it otherwise, and rejection of what T3 Code cannot carry
- [ ] 0.3 Tests against fixture settings files: the bytes and the modification time do not change, and no secret value reaches any output
- [ ] 0.4 Behavior proof under `docs/proofs/rune-installs-profiles-into-interfaces/`

## 1. Command and state record

- [ ] 1.1 `rune interface` with `list`, `enable`, `disable`, and `status`, routed from `src/cli/mod.rs`, with `--dry-run` and `--json`
- [ ] 1.2 State record and backups under `~/.local/state/rune/interfaces/`: owned keys, entries, wrappers, and backup paths
- [ ] 1.3 Tests: list output, unknown interface name, JSON stays machine-readable, no secret value in any output

## 2. Client step

- [ ] 2.1 Verify the reference forms for the hub key: Claude Code `apiKeyHelper`, Codex `env_key`. Record the findings in design.md before coding
- [ ] 2.2 Claude Code: back up, then write the hub URL and the key reference into the settings `env`, or into a second configuration directory with `--second-dir`; never write `CLAUDE_CODE_ENTRYPOINT`
- [ ] 2.3 Codex: back up, then write the hub model provider with the responses wire API, websockets, and the key reference, or into a second `CODEX_HOME`
- [ ] 2.4 Tests against fixture files: other keys unchanged, backups written once, no literal key, second directory leaves the normal files untouched

## 3. Cache lifetime

- [ ] 3.1 Write `promptCacheTtl: "1h"` on hub-routed Claude Code, and `subagentPromptCacheTtl: "1h"` with `--subagent-cache`
- [ ] 3.2 Refuse below Claude Code 2.1.242, naming the version
- [ ] 3.3 Tests: setting written, refusal on an old version, `FORCE_PROMPT_CACHING_5M` reported

## 4. T3 Code

- [ ] 4.1 Add the routed model names to the Claude entry's `customModels`, set `homePath` with a second directory, read the file again after the write
- [ ] 4.2 Find how T3 stores a secret reference for a usage source; until then add the hub URL only and tell the owner to paste the key in T3
- [ ] 4.3 Tests against fixture settings files: other settings unchanged, owned models removed on disable, the owner's own models kept

## 5. Multica

- [ ] 5.1 Inheritance check: Multica's Claude and Codex runtimes resolve the machine's CLIs
- [ ] 5.2 Launch wrappers only for profiles with arguments or middleware an interface cannot express, registered with `multica runtime profile create` and `set-path`
- [ ] 5.3 Tests with a fake `multica` executable on the path

## 6. Validator

- [ ] 6.1 Hub model list with the key, reusing the `rune launch --check` model comparison
- [ ] 6.2 Cache lifetime, session affinity (read-only, unknown when unreadable), Codex websockets
- [ ] 6.3 Warning for Claude subscription models routed into a harness other than Claude Code
- [ ] 6.4 Stale ACP entries in T3 Code and Multica, each with its removal step
- [ ] 6.5 Tests for each finding

## 7. Owner steps

- [ ] 7.1 Owner: sign every Claude and Codex account into the hub with CLIProxyAPI's login commands
- [ ] 7.2 Owner: enable T3 Code, start one Claude thread, pause over five minutes, and confirm `ephemeral_1h_input_tokens` in the usage
- [ ] 7.3 Owner: decide whether to keep a direct Claude login beside the hub, which picks the second directory option or not
- [ ] 7.4 Owner: remove the stale T3 entry `acpRegistry_rune` and the Multica profile `Rune (ACP)`
- [ ] 7.5 Owner: close Z-88 and Z-122, and accept the decision record in adr.md

## 8. Documentation and verification

- [ ] 8.1 README section and CHANGELOG entry
- [ ] 8.2 Draft ontology entries for interface, launch profile, and launch wrapper after the owner confirms the definitions
- [ ] 8.3 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, focused tests, `make validate`, `rune spec validate rune-installs-profiles-into-interfaces`
