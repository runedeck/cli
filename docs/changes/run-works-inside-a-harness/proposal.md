---
status: proposed
---

# Run works inside a harness

## Why

The runner fails when another harness starts it, which is how the owner uses it most. Three sessions in the Claude desktop app hit the same wall on 2026-10-20 (Proton/ai-demo, setup/git-ai, runedeck/drafts):

- Every claude route through the model proxy hung or exited 1. The child claude inherited the desktop session's own variables (`CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`, the host session id and its messaging socket). With those removed, `sol@claude` answered `ok` in 2.8 seconds.
- The real error never showed. On a nonzero exit the runner printed the first 500 characters of stderr and dropped stdout, where `claude --print` and `opencode run` write their errors. A timeout showed no output at all.
- `--json` printed plain text on some paths, and the dry run hid the flags the runner adds, so a session could not see what would start.
- A clean grok run refused to start without `~/.grok/auth.json`, although its route authenticates through the environment.
- Claude in workspace-write mode could edit but not run a shell command, so a Fable implementation run could not build or test its own work.
- A codex profile's model was reported as dropped while the same model went through `--model`.

The runner is meant to be a thin layer: it starts each harness the way the owner's own terminal would, with the route's model, endpoint, and credential, and reports what happened.

## What Changes

- The runner and the launcher remove the parent harness's session variables before they apply the route's environment. A route that sets an Anthropic or OpenAI endpoint without a credential gets no inherited credential for that endpoint.
- A failed or timed-out run reports the tail of both output streams, in text and in JSON.
- `--json` makes every outcome one JSON object on stdout, the dry run and configuration errors included.
- The dry run prints the full argument list the harness gets, the endpoint from the environment, and the environment keys with credential values hidden.
- A clean grok run starts without an authentication file when there is none.
- Claude in workspace-write mode runs shell commands inside Claude Code's own sandbox, and every claude run names its allowed tools, so a user hardening setting cannot drop the permission mode.
- A codex profile whose model the run passes anyway is not reported as dropped.
- A failure that looks like a sandbox denial carries a hint that names the sandbox.

## Capabilities

- execute-prompts-off-session (modified)

## Impact

- `src/cli/surface/mod.rs`, `src/cli/surface/tests.rs`, `src/cli/run/mod.rs`, `src/cli/launch/mod.rs`, `src/cli/process/mod.rs`, `tests/run.rs`, `README.md`, `CHANGELOG.md`.
- The council mod in runedeck/mods and the HarnessCouncil skill call the runner from inside a harness and stop needing the `env -u` and `SCRUB=0` workarounds.
