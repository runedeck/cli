# Launch Profiles Design

## Approach

`rune launch` resolves a named profile into the accepted middleware plan. The profile adds environment entries, arguments, middleware names, and an optional model-route alias. The resolved plan is shared with automated execution, while `rune launch` retains its inherited-terminal process backend.

## Structure

- `rune launch [profile@]<tool> [launch options] [-- tool args]` resolves `launch.profiles.<tool>.<profile>` from user configuration.
- A profile carries `env`, `args`, `with`, and optional `model` fields.
- Environment values are literals or `from_env` references. The parent environment is checked first, followed by the configured env file.
- `launch.models.<alias>` carries `id`, `context`, and optional `compact` metadata. A configured route replaces the complete built-in route with the same alias.
- Claude model routes derive `ANTHROPIC_MODEL`, `CLAUDE_CODE_MAX_CONTEXT_TOKENS`, and `CLAUDE_CODE_AUTO_COMPACT_WINDOW`. An explicit `compact` value also derives `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` for earlier compaction.
- A profile selecting a model route cannot define route-owned environment keys.
- Dry-run output reports the route alias, model id, context, source, generated settings, final arguments, middleware, and redacted credentials.
- Interactive launch executes the resolved plan through inherited input and output. Automated execution uses the same resolution with its supervised provider backend.

## Risks

- A profile can leak credentials through output. Credential-marker values remain redacted in dry-run environment and wrapped arguments.
- Model and context values can disagree. Route-owned keys are generated together and explicit conflicts fail resolution.
- A user-defined route can accidentally inherit stale built-in fields. Configured aliases replace complete built-in entries.
- Terminal wrappers can escape automated supervision. `rune run` rejects tmux and Docker wrappers while `rune launch` retains them.
