# Launch model check design

## Approach

`--check` is a third terminal mode of the launch resolver, beside spawn and `--dry-run`. It reuses `ResolvedLaunch` as it stands: the base URL comes from `plan.base_url` or the tool's base URL environment key, the credential from the resolved environment, and the model ids from the model route and the environment keys the tool reads. The check owns no new configuration. It beat a check on every launch, which would put a network round trip and a failure mode in front of every interactive start, and it beat a `rune doctor` section, which reads config and cannot know which credential a given profile sends.

## Structure

- `src/cli/launch/check.rs`. `plan_checks(&ResolvedLaunch)` collects, per base URL, the credential key and the model ids. `run_checks` performs the `GET /v1/models` calls through `ureq` with a five-second timeout and returns one `CheckReport`. `format_report` and the JSON shape live beside it.
- `src/cli/launch/mod.rs`. `parse_cli_tail` accepts `--check`. `execute_cli` routes it before `run_pre_steps`. `resolve_model` gains the `kimi` route. `credential_values` and `redact_credentials` are reused on every printed line.
- `src/cli/run/mod.rs`. `RunOptions.check` is set by the flag. The prompt sources are not read when it is set. The check uses the argv and model override the run would use.
- Model ids per tool. claude reads `ANTHROPIC_MODEL` and `ANTHROPIC_SMALL_FAST_MODEL`. codex reads the `-m`/`--model` argument and `model=` config arguments. Other tools contribute the route id only. The credential key per URL is `ANTHROPIC_AUTH_TOKEN`, then `ANTHROPIC_API_KEY`, then `OPENAI_API_KEY`, sent as `Authorization: Bearer` and, for Anthropic URLs, also as `x-api-key`.
- Tests stub the endpoint with a local `TcpListener` that answers a canned model list, a 401, and a body that echoes the token.

## Risks

- An endpoint that lists models it cannot serve for this credential passes the check. The report says "served", never "works". Task 2.2 records one live run.
- A proxy without `/v1/models` makes every check exit 2. That is the documented meaning of 2, and the launch itself is untouched.
- The Kimi K3 standard route limit is not published by the proxy. The built-in context is an assumption recorded in the ADR and overridable in config.
