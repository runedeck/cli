---
adr: docs/changes/convenient-cross-harness-launcher/adr.md
status: proposed
decisions: ["Built-in Claude Code profiles route through a local CLIProxyAPI", "CLI-0021 Launch Profile Composition", "CLI-0026 Route-Specific Model Metadata"]
---

# Launch Profiles

## Why

Claude Code reads its endpoint, its credential, its model, and its context window from environment variables ([CCENV]). To run Claude Code against another model through a local proxy such as CLIProxyAPI ([CLIPROXY]), a user sets `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, and `ANTHROPIC_MODEL` for each session. A model whose context window differs from the size Claude Code assumes for its name also needs `CLAUDE_CODE_MAX_CONTEXT_TOKENS` ([CCENV]). A group of variables set by hand can pair a model with the wrong window, and a token written into a config file can leak. A *launch profile* names the group once, keeps the token as a reference to the environment, and composes with the launch middleware chain.

## What Changes

- `rune launch [profile@]<tool>` resolves a launch profile from `launch.profiles.<tool>.<profile>` in the user config, or from the *built-in catalog*. A configured profile replaces the built-in profile of the same name.
- A profile carries environment entries, arguments, middleware names, and an optional *model route*. Its arguments come before the native arguments after `--`, and `--direct` drops its middleware.
- An environment value is a literal or a `from_env` reference. Rune reads a reference from the parent environment first and from the env file second. Dry-run output redacts credential values.
- A model route in `launch.models.<alias>` names a model id, a context size, and an optional compaction percentage. For Claude Code, the route sets the model and context variables as one group, and a profile that also sets one of them fails.
- The built-in catalog holds `sol@claude` and `grok@claude`. Both send Claude Code to CLIProxyAPI at `http://127.0.0.1:8317` with the token from `CLIPROXY_API_KEY`.
- Bare `rune launch` lists each known tool, its install state, and its `profile@tool` invocations. For `ollama`, a name with no profile is a model: `rune launch llama3@ollama` runs `ollama run llama3`.
- `rune launch` runs the tool with the terminal's input and output and exits with its status. `rune run` resolves the same plan.

## Capabilities

### Modified Capabilities

- `convenient-cross-harness-launcher`: named profiles, the built-in catalog, model routes, and interactive execution.

## Impact

- `src/ontology.rs` holds the profile, route, and built-in catalog types. `src/cli/launch/mod.rs` holds resolution, dry-run output, redaction, and execution.
- The `launch` help text, `README.md`, and `config.example.yaml` show launch and run examples.
- The proof under `docs/proofs/convenient-cross-harness-launcher/` runs against the fixture in `tests/fixtures/convenient-cross-harness-launcher/`.

[CCENV]: https://code.claude.com/docs/en/env-vars
[CLIPROXY]: https://github.com/router-for-me/CLIProxyAPI
