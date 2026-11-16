# Rune installs profiles into interfaces design

## Approach

Theo's CLIProxyAPI hub guide is the baseline ([THEO]). Each machine's Claude Code and Codex configuration
points at the hub, and T3 Code and Multica inherit it because they run those CLIs with the machine's
configuration. rune fills four gaps: the client step from launch profiles, the one-hour cache, the routed
model names in T3, and the checks. One wrapper per profile was dropped: the profiles share one base URL and
one key reference and differ only in model and small fast model (owner, 2026-11-02), and the hub routes by
model name, so T3's model picker covers them.

## Structure

### Commands

| Command | Behavior |
| --- | --- |
| `rune interface` or `rune interface list` | Known interfaces, whether each is enabled, and the client step state |
| `rune interface enable t3code [--second-dir] [--subagent-cache] [--usage-source] [--dry-run] [--json]` | Client step when missing, cache lifetime, routed models in T3's Claude entry, optional usage source |
| `rune interface enable multica [--dry-run] [--json]` | Client step when missing, then the inheritance check. Wrappers only for profiles that need them |
| `rune interface disable <name> [--dry-run] [--json]` | Removes what the state record lists, from backups and owned keys |
| `rune interface status [<name>] [--json]` | The validator checks below |

`rune --help` (0.6.0) has no `interface` command.

### Client step

| Client | What the deployer writes (from [THEO] step 6) |
| --- | --- |
| Claude Code `~/.claude/settings.json`, or `CLAUDE_CONFIG_DIR` for the second directory | `env.ANTHROPIC_BASE_URL` = hub URL. The key through a secret reference, see below |
| Codex `~/.codex/config.toml`, or `CODEX_HOME` for the second directory | `model_provider = "cliproxy"` and `[model_providers.cliproxy]` with `base_url = <hub>/v1`, `wire_api = "responses"`, `requires_openai_auth = false`, `supports_websockets = true`, and the key through a secret reference |

The guide writes the key as a literal (`ANTHROPIC_AUTH_TOKEN`, `experimental_bearer_token`). rune writes a
reference instead. Candidates, both to verify in task 2.1 before coding: Claude Code's `apiKeyHelper` command
that prints the key from rune's env file, and Codex's `env_key` field naming an exported variable. If a
client offers no reference form, the deployer stops and reports it.

Today the owner's Codex provider `cliproxyapi` holds `base_url`, `name`, and `wire_api` only, with no
websockets setting (`~/.codex/config.toml:435-438`, read 2026-11-02).

### Cache lifetime

Claude Code reads `promptCacheTtl` or `CLAUDE_CODE_PROMPT_CACHE_TTL` for the main conversation and
`subagentPromptCacheTtl` or `CLAUDE_CODE_SUBAGENT_PROMPT_CACHE_TTL` for the rest. Values are `5m` or `1h`.
All four need Claude Code 2.1.242 or later. `FORCE_PROMPT_CACHING_5M=1` overrides them. Through a gateway the
one-hour request travels partly in the `anthropic-beta` header, which the gateway must forward [CACHE]. The
deployer writes `promptCacheTtl: "1h"`, and `subagentPromptCacheTtl: "1h"` with `--subagent-cache`. Cache
writes show up as `ephemeral_1h_input_tokens` [CACHE], which task 7.2 uses for the live check.

### T3 Code

- Settings live in `~/.t3/userdata/settings.json`, or `$T3CODE_HOME/userdata/settings.json`. Provider
  entries are `{driver, displayName?, accentColor?, environment?, enabled?, config?}`
  (`packages/contracts/src/providerInstance.ts:75-82` in pingdotgg/t3code at `47f2c8d29e`).
- The Claude entry's `config.customModels` takes the routed model names (`packages/contracts/src/settings.ts:731`).
  The owner's entry today is `claudeAgent` with `launchArgs: --settings ~/.config/t3/claude-settings.json`.
  Values in that file override user settings, so status checks that it sets neither the hub URL nor the
  cache lifetime to something else.
- With a second directory, the entry's `config.homePath` names it. T3's import reads `homePath` or the
  entry's `CLAUDE_CONFIG_DIR` and `CODEX_HOME` (`apps/server/src/project/AgentSessionScanner.ts:1302-1331`).
- Codex stays on "Use existing CLI", which reads `config.toml` [THEO].
- Write path: read the file, change only the owned keys, write a temporary file in the same directory, rename
  it over the original, read it again. T3 watches the file and reloads it after a 100 ms debounce
  (`apps/server/src/serverSettings.ts:1271-1281`), and rereads it before each of its own writes
  (`serverSettings.ts:1137` and `:1185`). A write while T3 runs can lose an update: see "T3 findings". rune never opens `statev2.sqlite`.
- Usage source: T3 already has `usageLimitSources.cliproxy-localhost-8317` with its key kept in T3's secret
  store (`~/.t3/userdata/secrets/usage-limit-source-*.bin`). How T3 encodes a secret reference for a new
  source is open (task 4.2). Until then the deployer adds the URL and asks the owner to paste the key in T3.
- No `t3` subcommand edits provider entries. T3's settings RPC `providerInstanceMutation`
  (`packages/contracts/src/rpc.ts:825`) needs an authenticated session, so rune uses the file.

### Entrypoint variable

T3 sets `CLAUDE_CODE_ENTRYPOINT=t3-code` for the Claude processes it starts. A value in the entry's
environment or in Claude's settings `env` overrides it (`apps/server/src/provider/Drivers/ClaudeHome.ts:69-85`).
The deployer edits Claude's settings `env`, so it must never add that variable there.

### Multica

Multica's daemon runs the machine's `claude` and `codex`, so its runtimes inherit the client step. A launch
wrapper is the exception, for a profile with arguments or middleware that T3 or Multica cannot express. The
existing pattern is the Multica profile "Kimi K3 via Rune (Claude)": command `rune` with fixed arguments
`launch kimi@claude --` on the claude protocol. The CLI cannot set fixed arguments, so rune writes a wrapper
file and registers it with `multica runtime profile create --command-name` and `set-path` (CLI 0.7.2).

### Validator checks

| Check | How |
| --- | --- |
| Hub model list | `GET <hub>/v1/models` with the key, as [THEO] verifies, compared with the routed models. Reuses `rune launch --check` |
| Cache lifetime | Effective `promptCacheTtl` or its variable is `1h`, and `FORCE_PROMPT_CACHING_5M` is unset |
| Session affinity | `routing.session-affinity: true` in the hub configuration [THEO], read-only. Unknown when unreadable |
| Codex websockets | `supports_websockets = true` in the Codex hub provider |
| Claude subscription models | Warning for a non-Claude-Code profile or interface entry that routes a Claude model through the hub, [THEO] hard rule 2 |
| Stale ACP | T3 entry with driver `acpRegistry` and command `rune` (`acpRegistry_rune` today). Multica hermes-family profile with command `rune` (`Rune (ACP)`, `e5be1b22-9009-48ef-8e7b-9bfe1a89993e`) |

### Out of scope, with the tool that owns it

| Concern | CLIProxyAPI's own tool [THEO] |
| --- | --- |
| Install the hub | `brew install cliproxyapi`, the cliproxyapi-installer script, the Docker image |
| Keep it running | `brew services start cliproxyapi`, the systemd user service |
| Account logins | `cli-proxy-api -claude-login`, `-codex-device-login`, `-codex-login`, browser OAuth by the owner |
| Routing strategy and soonest-reset routing | Hub `routing` settings, per-account `priority` and `weight`, or a fork |
| Dashboards | The management panel at `/management.html`, or `-tui` |

## T3 findings

A study of the T3 Code source at `47f2c8d29e` on 2026-11-15 changes four points of this design. The read-only
slice of 0.7.0 follows them.

- Named models go into `providerInstances.<instance>.config.customModels`. A model there is a string, or an
  object with `slug` and an optional `name`. The plan uses the object form, with the profile name as `name`.
  Models that the list already holds stay, and so does every key that rune does not know.
- A profile without environment values adds its model to the native instance of its tool, `claudeAgent` or
  `codex`. A profile with a base URL or any other environment value is proxy-backed. Its models go into a
  provider instance of its own, beside the native one, and the native instance is never repointed. Profiles of
  one tool with the same environment share one instance, named `rune_<tool>_<first profile>`.
- The `environment` of an instance is a list of `{name, value, sensitive}`. A stored secret is a reference
  beside an empty value, and T3 Code keeps the value in its own store. So a `from_env` value of a profile stays
  a reference in the plan. The plan names the variable and marks it sensitive, and the owner enters the value
  in T3 Code.
- The earlier text said that a write is safe while T3 runs. T3 Code has a file watcher and a write lock of its
  own, and shares no lock with rune. A rename over the file prevents a torn file. It does not prevent a lost
  update between two writers. The write change must read the file again before it replaces it, stop when the
  file changed, and report the conflict.

The planner reads profile data only. It resolves no secret and runs no middleware. It rejects a profile whole
when T3 Code cannot carry it: a tool with no T3 Code driver, middleware (`with`), arguments, an unknown model
route, a URL with a user name or a password, or a base URL with a query string or a fragment, which can hold
a key. For an accepted profile it names what T3 Code loses, such as the
context window of a Claude route. A dry run shows a value only for the base URL. Every other literal and every
reference stays out of the output.

| Command | Reads | Writes |
| --- | --- | --- |
| `rune interface list` | whether the settings file exists | nothing |
| `rune interface status t3code` | the settings file and the launch profiles | nothing |
| `rune interface enable t3code --dry-run` | the settings file and the launch profiles | nothing |
| `rune interface enable t3code` | nothing | nothing: it refuses |

## Terms for the owner

None of these terms has an entry in `deck/ontology/rune.ttl`. Proposed definitions:

- **interface**: a program that a person uses to start and watch harness sessions. It runs harness binaries
  but loads no instructions itself.
- **launch profile**: a named set of environment values, arguments, and middleware that rune applies when it
  starts one harness.
- **launch wrapper**: a small executable that rune writes for one launch profile, for an interface to run in
  place of the harness binary when the interface cannot express the profile.

## Risks

- A client drops the reference form for keys. The deployer stops instead of writing a literal.
- The hub stops forwarding the `anthropic-beta` header. The live check in task 7.2 shows five-minute writes.
- T3's one Claude entry means one small fast model per machine. Profiles that differ only there lose that
  difference inside T3.
- A concurrent T3 save drops rune's keys. The read after the write catches it.

[THEO]: https://gist.github.com/t3dotgg/59a521d2ee8d70aaf90e280c175072af
[CACHE]: https://code.claude.com/docs/en/prompt-caching#choose-the-ttl-yourself
