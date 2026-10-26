# Launch Profiles Design

## Approach

`rune launch` resolves a named profile into the middleware plan. The profile adds environment entries, arguments, middleware names, and an optional model-route alias. `rune run` reads the same resolved plan, and `rune launch` runs it with the terminal's input and output.

## Structure

- `rune launch [profile@]<tool> [launch options] [-- tool args]` resolves `launch.profiles.<tool>.<profile>` from the user configuration, laid over the built-in catalog.
- The built-in catalog defines the `claude` profiles `sol` and `grok`. Each sets `ANTHROPIC_BASE_URL` from `launch.middleware.cliproxy`, reads `ANTHROPIC_AUTH_TOKEN` from `CLIPROXY_API_KEY`, adds the `cliproxy` middleware, and selects the route of its name.
- A configured profile replaces the built-in profile of the same name. A profile carries `env`, `args`, `with`, and optional `model` fields.
- Environment values are literals or `from_env` references. Rune reads the parent environment first and the configured env file second.
- `--direct` clears the default middleware chain and the profile's `with` entries. The profile's environment and arguments stay.
- `launch.models.<alias>` carries `id`, `context`, and optional `compact` metadata. A configured route replaces the complete built-in route with the same alias.
- Claude model routes derive `ANTHROPIC_MODEL`, `CLAUDE_CODE_MAX_CONTEXT_TOKENS`, and `CLAUDE_CODE_AUTO_COMPACT_WINDOW`. An explicit `compact` value also derives `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` for earlier compaction.
- A Claude profile selecting a model route cannot define route-owned environment keys.
- Dry-run output reports the route alias, model id, context, source, generated settings, final arguments, middleware, and redacted credentials.
- Interactive launch runs the resolved plan with inherited input and output. Automated execution uses the same resolution with its supervised provider backend.

## Risks

- A profile can leak credentials through output. Credential-marker values remain redacted in dry-run environment and wrapped arguments.
- Model and context values can disagree. Route-owned keys are generated together and explicit conflicts fail resolution.
- A user-defined route can accidentally inherit stale built-in fields. Configured aliases replace complete built-in entries.
- Built-in profiles point at one local port. A proxy that listens elsewhere needs `launch.middleware.cliproxy.host` and `port`, which move the base URL and the readiness check together.
- Terminal wrappers can escape automated supervision. `rune run` rejects tmux and Docker wrappers while `rune launch` keeps them.
