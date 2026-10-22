---
type: proof
change: run-works-inside-a-harness
head: 1c811b81f919ebbd259aafe6061fe280cfee2cef
recorded: 2026-10-22
transcript: c2f72a1e5c2821c843395e3cef601c5e9936f37e32711e8aa506ea77db567670
scenes:
- scenario: execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/desktop-session-starts-a-proxied-claude-route
  kind: check
- scenario: execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/codex-session-starts-another-harness
  kind: check
- scenario: execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/route-endpoint-without-a-credential
  kind: unproven
- scenario: execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/native-claude-without-a-route-endpoint
  kind: unproven
- scenario: execute-prompts-off-session#a-failed-run-shows-what-the-harness-said/claude-prints-its-error-on-stdout
  kind: check
- scenario: execute-prompts-off-session#a-failed-run-shows-what-the-harness-said/run-times-out
  kind: check
- scenario: execute-prompts-off-session#json-output-is-one-object-for-every-outcome/dry-run-as-json
  kind: check
- scenario: execute-prompts-off-session#json-output-is-one-object-for-every-outcome/unknown-profile-as-json
  kind: check
- scenario: execute-prompts-off-session#the-dry-run-shows-what-would-start/endpoint-set-through-the-environment
  kind: check
- scenario: execute-prompts-off-session#the-dry-run-shows-what-would-start/credential-in-the-plan
  kind: check
- scenario: execute-prompts-off-session#clean-state-bridges-only-what-exists/grok-route-authenticates-through-the-environment
  kind: unproven
- scenario: execute-prompts-off-session#claude-workspace-write-runs-shell-commands-in-a-sandbox/implementation-run-builds-its-work
  kind: unproven
- scenario: execute-prompts-off-session#claude-workspace-write-runs-shell-commands-in-a-sandbox/user-settings-set-the-subprocess-scrub
  kind: unproven
- scenario: execute-prompts-off-session#a-failure-caused-by-a-sandbox-says-so/proxy-certificate-fails-inside-a-sandbox
  kind: check
- scenario: execute-prompts-off-session#a-failure-caused-by-a-sandbox-says-so/model-text-quotes-a-denial
  kind: check
- scenario: execute-prompts-off-session#the-check-trusts-the-roots-the-environment-names/proxy-root-named-by-the-environment
  kind: unproven
- scenario: execute-prompts-off-session#a-dropped-profile-model-equal-to-the-run-s-model-is-not-reported/codex-profile-names-the-routed-model
  kind: unproven
- scenario: execute-prompts-off-session#a-dropped-profile-model-equal-to-the-run-s-model-is-not-reported/codex-profile-names-another-model
  kind: unproven
- scenario: execute-prompts-off-session#a-dropped-profile-model-equal-to-the-run-s-model-is-not-reported/claude-settings-carry-a-permission-grant
  kind: unproven
---

# Proof: run-works-inside-a-harness

One scene per scenario, in closure order. The proof was recorded from inside a Claude Code session, so the parent's `CLAUDECODE`, `CLAUDE_CODE_*` and `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` variables are in the recorder's environment, and the dry runs list them under `env_removed:`. A failure scene builds a fake harness in its own folder and runs the route with `--binary`, so no model is called for it. The scenes that need a route, a profile, or a certificate this host cannot give name what they need.

## execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/desktop-session-starts-a-proxied-claude-route

The dry run shows the route's endpoint, model, and credential (redacted) and lists the parent session's variables the child never sees. The live run answers through the route.

```console
$ rune run sol@claude --dry-run "Reply with exactly two words: proof ready"
tool: claude
model: sol
model_id: [..]
...
env:
...
  ANTHROPIC_AUTH_TOKEN=<redacted>
  ANTHROPIC_BASE_URL=[..]
...
env_removed:
...
  CLAUDECODE
...
  CLAUDE_CODE_ENTRYPOINT
...
  CLAUDE_CODE_SUBPROCESS_ENV_SCRUB
...
base_url: [..]
...
$ rune run sol@claude "Reply with exactly two words: proof ready"
...
proof ready
```

## execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/codex-session-starts-another-harness

With codex's sandbox variables in the parent, the plan lists them among the removed keys.

```console
$ CODEX_SANDBOX=seatbelt CODEX_SANDBOX_NETWORK_DISABLED=1 rune run sol@claude --dry-run "x"
tool: claude
...
env_removed:
...
  CODEX_SANDBOX
...
  CODEX_SANDBOX_NETWORK_DISABLED
...
```

## execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/route-endpoint-without-a-credential

Needs: a claude profile that sets an Anthropic endpoint and no credential. The unit test `an_inherited_credential_of_the_route_family_is_withheld` covers it.

```console
```

## execute-prompts-off-session#the-child-harness-starts-free-of-the-parent-harness-s-session/native-claude-without-a-route-endpoint

Needs: plain `claude` with an Anthropic endpoint and credential in the parent. The unit tests of the isolation cover it.

```console
```

## execute-prompts-off-session#a-failed-run-shows-what-the-harness-said/claude-prints-its-error-on-stdout

A harness that exits 1 with an API error on stdout and a warning on stderr: the error names the exit, the harness error, and both tails.

```console
$ sh -c 'printf "%s\n" "#!/bin/sh" "echo API Error: 400 bad request" "echo warning: model deprecated >&2" "exit 1" > fake-claude && chmod 755 fake-claude'
$ rune run sol@claude --binary ./fake-claude "x"
? 1
error: process Exited(1)
harness errors:
  API Error: 400 bad request
stdout tail:
  API Error: 400 bad request
stderr tail:
  warning: model deprecated
```

## execute-prompts-off-session#a-failed-run-shows-what-the-harness-said/run-times-out

A harness that wrote part of its answer before the deadline: the timeout result carries that tail, in text and in JSON.

```console
$ sh -c 'printf "%s\n" "#!/bin/sh" "echo partial answer" "sleep 5" > slow-claude && chmod 755 slow-claude'
$ rune run sol@claude --binary ./slow-claude --timeout 1s "x"
? 124
error: [..]
...
stdout tail:
  partial answer
...
$ rune --json run sol@claude --binary ./slow-claude --timeout 1s "x"
? 124
{"details":{[..]"stdout_tail":"partial answer[..]},[..]"kind":"timeout","ok":false,[..]}
```

## execute-prompts-off-session#json-output-is-one-object-for-every-outcome/dry-run-as-json

```console
$ rune --json run sol@claude --dry-run "x"
{"argv":[..],"base_url":"[..]",[..]"env":{[..]"ANTHROPIC_AUTH_TOKEN":"<redacted>",[..]},"env_removed":[[..]"CLAUDECODE"[..]],[..]"kind":"dry_run",[..]"ok":true,[..]}
```

## execute-prompts-off-session#json-output-is-one-object-for-every-outcome/unknown-profile-as-json

```console
$ rune --json run nosuch@claude "x"
? 2
{"details":{"message":"no launch profile 'nosuch' for claude (profiles: [..])","profiles":[[..]]},"kind":"configuration_error","ok":false,"tool":"nosuch@claude"}
```

## execute-prompts-off-session#the-dry-run-shows-what-would-start/endpoint-set-through-the-environment

The `sol@claude` profile sets its endpoint only through `ANTHROPIC_BASE_URL` in its environment, and the plan names it as `base_url`.

```console
$ rune run sol@claude --dry-run "x"
tool: claude
...
  ANTHROPIC_BASE_URL=[..]
...
base_url: http[..]
...
```

## execute-prompts-off-session#the-dry-run-shows-what-would-start/credential-in-the-plan

```console
$ rune run sol@claude --dry-run "x"
tool: claude
...
  ANTHROPIC_AUTH_TOKEN=<redacted>
...
```

## execute-prompts-off-session#clean-state-bridges-only-what-exists/grok-route-authenticates-through-the-environment

Needs: grok@grok on a host without `~/.grok/auth.json`. grok's own sandbox cannot start inside the recording sandbox.

```console
```

## execute-prompts-off-session#claude-workspace-write-runs-shell-commands-in-a-sandbox/implementation-run-builds-its-work

Needs: sol@claude in workspace-write mode outside the recording sandbox, where Claude Code's own sandbox can run a shell command.

```console
```

## execute-prompts-off-session#claude-workspace-write-runs-shell-commands-in-a-sandbox/user-settings-set-the-subprocess-scrub

Needs: sol@claude with the owner's settings. The first scene's stderr shows the scrub warning and the run still answers, because the run names its allowed tools.

```console
```

## execute-prompts-off-session#a-failure-caused-by-a-sandbox-says-so/proxy-certificate-fails-inside-a-sandbox

A harness whose stderr carries `-26276` fails with a hint that names the outer sandbox.

```console
$ sh -c 'printf "%s\n" "#!/bin/sh" "echo error: certificate verify failed, OSStatus -26276 >&2" "exit 1" > fake-claude && chmod 755 fake-claude'
$ rune run sol@claude --binary ./fake-claude "x"
? 1
error: process Exited(1)
...
stderr tail:
  error: certificate verify failed, OSStatus -26276
hint: [..]
```

## execute-prompts-off-session#a-failure-caused-by-a-sandbox-says-so/model-text-quotes-a-denial

A denial quoted on stdout alone earns no hint.

```console
$ sh -c 'printf "%s\n" "#!/bin/sh" "echo the tool said Operation not permitted" "exit 1" > fake-claude && chmod 755 fake-claude'
$ rune run sol@claude --binary ./fake-claude "x"
? 1
error: process Exited(1)
stdout tail:
  the tool said Operation not permitted
```

## execute-prompts-off-session#the-check-trusts-the-roots-the-environment-names/proxy-root-named-by-the-environment

Needs: the proxy's root certificate in a PEM file named by `NODE_EXTRA_CA_CERTS`, and the proxy reachable over TLS.

```console
```

## execute-prompts-off-session#a-dropped-profile-model-equal-to-the-run-s-model-is-not-reported/codex-profile-names-the-routed-model

Needs: a codex profile that sets `model` to the routed model. The unit test `a_codex_model_equal_to_the_runs_model_is_dropped_without_a_warning` covers it.

```console
```

## execute-prompts-off-session#a-dropped-profile-model-equal-to-the-run-s-model-is-not-reported/codex-profile-names-another-model

Needs: a codex profile that sets `model` to another model. The unit tests of the profile filter cover it.

```console
```

## execute-prompts-off-session#a-dropped-profile-model-equal-to-the-run-s-model-is-not-reported/claude-settings-carry-a-permission-grant

Needs: a claude profile whose `--settings` object holds `permissions`. The unit test `drop_warnings_name_the_key_and_never_the_value` covers it.

```console
```
