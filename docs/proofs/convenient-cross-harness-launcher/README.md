---
type: proof
change: convenient-cross-harness-launcher
recorded: 2026-10-26
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
- scenario: convenient-cross-harness-launcher#named-launch-profiles/built-in-proxy-profile
  kind: unproven
- scenario: convenient-cross-harness-launcher#named-launch-profiles/profile-environment-and-arguments
  kind: unproven
- scenario: convenient-cross-harness-launcher#named-launch-profiles/secret-reference
  kind: unproven
- scenario: convenient-cross-harness-launcher#named-launch-profiles/bare-launch-lists-profiles
  kind: unproven
- scenario: convenient-cross-harness-launcher#named-launch-profiles/ollama-model-fallback
  kind: unproven
- scenario: convenient-cross-harness-launcher#route-specific-model-metadata/claude-model-route
  kind: unproven
- scenario: convenient-cross-harness-launcher#route-specific-model-metadata/generated-setting-conflict
  kind: unproven
- scenario: convenient-cross-harness-launcher#route-specific-model-metadata/configured-route-replacement
  kind: unproven
- scenario: convenient-cross-harness-launcher#interactive-execution/native-session-behavior
  kind: unproven
---

# Proof: convenient-cross-harness-launcher

The scenes prove the launch profiles of #24, "feat: add Sol and Grok proxy profiles", against the real `rune` binary. Each scene runs `rune launch` with `HOME=.` in its scratch directory, so a scene without the fixture is a fresh installation. The fixture in `tests/fixtures/convenient-cross-harness-launcher/` holds a rune config with configured profiles and routes, the env file it names, and `bin/claude`, a stub that prints its arguments, one variable, and its standard input, then exits 3. Its `home.sh` copies them into the scene directory and makes the stub executable. Every launch except the last uses `--dry-run`, and the last one starts the stub in place of Claude Code, so no scene calls a model or a proxy. The dry-run `env_removed` block and the `base_url` of a profile that sets none depend on the parent environment, so the scenes elide them. Every scene is proven.

## convenient-cross-harness-launcher#named-launch-profiles/built-in-proxy-profile

```console
$ CLIPROXY_API_KEY=proof-key HOME=. rune launch sol@claude --dry-run
tool: claude
model: sol
model_id: gpt-5.6-sol
model_context: 272000
model_source: built-in
argv: claude
env:
  ANTHROPIC_MODEL=gpt-5.6-sol
  CLAUDE_CODE_MAX_CONTEXT_TOKENS=272000
  CLAUDE_CODE_AUTO_COMPACT_WINDOW=272000
  ANTHROPIC_AUTH_TOKEN=<redacted>
  ANTHROPIC_BASE_URL=http://127.0.0.1:8317
  ANTHROPIC_SMALL_FAST_MODEL=gpt-5.6-luna
...
base_url: http://127.0.0.1:8317
wrap:
pre:
  cliproxy 127.0.0.1:8317 <check-only>
$ CLIPROXY_API_KEY=proof-key HOME=. rune launch grok@claude --dry-run
tool: claude
model: grok
model_id: grok-4.6
model_context: 500000
model_source: built-in
...
  ANTHROPIC_AUTH_TOKEN=<redacted>
  ANTHROPIC_BASE_URL=http://127.0.0.1:8317
  ANTHROPIC_SMALL_FAST_MODEL=grok-composer-2.5-fast
...
  cliproxy 127.0.0.1:8317 <check-only>
$ env -u CLIPROXY_API_KEY HOME=. rune launch grok@claude --dry-run
? 2
fatal: profile references CLIPROXY_API_KEY, unset in the environment and absent from ./.env
```

## convenient-cross-harness-launcher#named-launch-profiles/profile-environment-and-arguments

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/convenient-cross-harness-launcher/home.sh"'
$ HOME=. rune launch review@claude --dry-run -- --resume 0b5e-proof
tool: claude
argv: claude --permission-mode plan --resume 0b5e-proof
env:
  OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318
  OTEL_SERVICE_NAME=rune-launch
  PROOF_MODE=review
...
base_url: [..]
wrap:
pre:
$ HOME=. rune launch review@claude --direct --dry-run -- --resume 0b5e-proof
tool: claude
argv: claude --permission-mode plan --resume 0b5e-proof
env:
  PROOF_MODE=review
...
base_url: [..]
wrap:
pre:
```

## convenient-cross-harness-launcher#named-launch-profiles/secret-reference

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/convenient-cross-harness-launcher/home.sh"'
$ HOME=. rune launch origin@claude --dry-run
tool: claude
argv: claude
env:
  ANTHROPIC_AUTH_TOKEN=<redacted>
  PROOF_ORIGIN=env-file
...
base_url: [..]
wrap:
pre:
$ HOME=. PROOF_ORIGIN=parent rune launch origin@claude --dry-run
...
  ANTHROPIC_AUTH_TOKEN=<redacted>
  PROOF_ORIGIN=parent
...
$ HOME=. rune launch absent@claude --dry-run
? 2
fatal: profile references PROOF_ABSENT_TOKEN, unset in the environment and absent from proof.env
```

## convenient-cross-harness-launcher#named-launch-profiles/bare-launch-lists-profiles

```console
$ HOME=. rune launch
 launchable tools
   claude     [..]  grok@claude sol@claude
   codex      [..]
   agy        [..]
   opencode   [..]
   grok       [..]
   cursor     [..]  fable@cursor
   ollama     [..]
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/convenient-cross-harness-launcher/home.sh"'
$ HOME=. rune launch
 launchable tools
   claude     [..]  absent@claude clash@claude early@claude grok@claude origin@claude review@claude sol@claude
...
```

## convenient-cross-harness-launcher#named-launch-profiles/ollama-model-fallback

```console
$ HOME=. rune launch llama3@ollama --dry-run
tool: ollama
argv: ollama run llama3
env:
...
base_url: [..]
wrap:
pre:
```

## convenient-cross-harness-launcher#route-specific-model-metadata/claude-model-route

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/convenient-cross-harness-launcher/home.sh"'
$ CLIPROXY_API_KEY=proof-key HOME=. rune launch grok@claude --dry-run
tool: claude
model: grok
model_id: grok-4.6
model_context: 500000
model_source: built-in
argv: claude
env:
  ANTHROPIC_MODEL=grok-4.6
  CLAUDE_CODE_MAX_CONTEXT_TOKENS=500000
  CLAUDE_CODE_AUTO_COMPACT_WINDOW=500000
  ANTHROPIC_AUTH_TOKEN=<redacted>
  ANTHROPIC_BASE_URL=http://127.0.0.1:8317
  ANTHROPIC_SMALL_FAST_MODEL=grok-composer-2.5-fast
...
base_url: http://127.0.0.1:8317
wrap:
pre:
  cliproxy 127.0.0.1:8317 <check-only>
$ HOME=. rune launch early@claude --dry-run
tool: claude
model: sol-early
model_id: gpt-5.6-sol
model_context: 272000
model_source: config
argv: claude
env:
  ANTHROPIC_MODEL=gpt-5.6-sol
  CLAUDE_CODE_MAX_CONTEXT_TOKENS=272000
  CLAUDE_CODE_AUTO_COMPACT_WINDOW=272000
  CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=85
...
base_url: [..]
wrap:
pre:
```

## convenient-cross-harness-launcher#route-specific-model-metadata/generated-setting-conflict

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/convenient-cross-harness-launcher/home.sh"'
$ HOME=. rune launch clash@claude --dry-run
? 2
fatal: launch profile model 'sol' owns generated environment keys: ANTHROPIC_MODEL; remove them from the profile env
```

## convenient-cross-harness-launcher#route-specific-model-metadata/configured-route-replacement

```console
$ CLIPROXY_API_KEY=proof-key HOME=. rune launch sol@claude --dry-run
tool: claude
model: sol
model_id: gpt-5.6-sol
model_context: 272000
model_source: built-in
...
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/convenient-cross-harness-launcher/home.sh"'
$ CLIPROXY_API_KEY=proof-key HOME=. rune launch sol@claude --dry-run
tool: claude
model: sol
model_id: gpt-5.6-sol-proof
model_context: 128000
model_source: config
argv: claude
env:
  ANTHROPIC_MODEL=gpt-5.6-sol-proof
  CLAUDE_CODE_MAX_CONTEXT_TOKENS=128000
  CLAUDE_CODE_AUTO_COMPACT_WINDOW=128000
...
```

## convenient-cross-harness-launcher#interactive-execution/native-session-behavior

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/convenient-cross-harness-launcher/home.sh"'
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. exec rune launch review@claude -- --resume 0b5e-proof'
< hello from the terminal
? 3
claude --permission-mode plan --resume 0b5e-proof
PROOF_MODE=review
stdin: hello from the terminal
```
