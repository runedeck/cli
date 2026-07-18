# Launch Profiles Design

## Approach

`rune launch` already implements the CLI-0018 middleware chain (`--with`, `--direct`, `--tmux`, `--dry-run`, sensitive-env redaction). Profiles ADD named environment presets on top and compose with middleware; the rejected alternative was replacing the chain with a profile-only launcher, which would discard an accepted architecture and its script extension contract. An amending ADR records the composition.

## Structure

- `rune launch <tool>[@profile] [launch options] [-- tool args]`; `claude@sol` resolves profile `sol` for tool `claude`.
- Profiles live under `launch.profiles.<tool>.<name>` in `~/.config/rune/config.yaml`; a repo's `defaults.yaml` may add non-sensitive profiles. Precedence: built-in < repo defaults (restricted) < user config < CLI flags.
- A profile carries `env:` (map), `args:` (list prepended to tool args), `with:` (middleware appended to the chain). Env values support `from_env: KEY` references; literal secret-looking values fail validation.
- Repo-defined profiles may not set credential, endpoint, proxy, certificate, `PATH`, loader, `HOME`, or `XDG_*` variables; violations are hard config errors naming the key.
- Bare `rune launch` lists tools (claude, codex, agy, opencode, grok, ollama) with install state and defined profiles, then exits 0.
- `ollama` is a REPL tool: `rune launch ollama@llama3` execs `ollama run llama3`; the profile name doubles as the model when no profile matches.
- Pre-exec freshness check compares the bound target's `.manifest` provenance commit against deck HEAD; a mismatch prints one dim warning with a `rune install` hint. Never blocks, never auto-installs.
- Commented profile templates for known setups (Anthropic model pins, OpenAI-compatible base URL, Bedrock/Vertex) ship in the config template.

## Risks

- Env injection from untrusted repos: countered by the restricted-key hard errors and the no-secrets-in-repo rule.
- Secret leakage in output: `SENSITIVE_ENV_KEYS` redaction extends to profile-resolved values in list, JSON, and dry-run paths.
- Profile/model ambiguity for ollama: profile lookup first, model fallback second; the plan output names which path was taken.
