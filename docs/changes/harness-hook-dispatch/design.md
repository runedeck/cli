# Design

The research, the vendor protocol table, and Astra's schematics live in the workshop under `docs/specs/2026-09-27-rune-hooks/`. This file holds what the code needs.

## Runtime

```text
harness event, stdin JSON
        |
rune hook run --harness H --native-event N
        |
read the compiled plan ~/.local/state/rune/hooks/plan.json (written by rune install;
a user config newer than the plan is a warning on stderr, never a reload)
if H is claude and Grok's environment is present: exit 0, log, stop
check argument against the payload's event name
normalize -> HandlerPayload v1
        |
for handler in plan[H][event], serial, ordered:
    deadline = min(handler.timeout, remaining - reserve); none left -> failure by policy, no spawn
    spawn exec, payload on stdin, RUNE_HARNESS RUNE_EVENT RUNE_HANDLER_ID RUNE_REMAINING_MS
    kill the process group on expiry
    parse one JSON Result; anything else is failure by policy (warn: log; deny: Result deny)
    a replacement input becomes the effective input for the handlers after it
    gate: stop on deny   collect: merge context in order, keep a deny   passive: ignore effects
        |
dispatcher failure (mismatch, decode, plan) -> strongest policy among subscribed handlers
encode Result for H and N -> stdout JSON, exit 0
```

## Modules

- `src/cli/hook/config.rs`: `HooksConfig { defaults, events, handlers }` from the `hooks:` key of the user config. `Handler { id, exec, events, order, timeout_ms, on_failure, requires, per_harness, rewrites, adopts }`. Validation names the field: empty events, unknown event or capability, `deny` on a non-gate event, a rewriter after a guard.
- `src/cli/hook/plan.rs`: the compiled plan per harness and event, with clamped timeouts. `rune hook list` and `--events` print it.
- `src/cli/hook/events.rs`: `Canonical` (eight), `native(harness)`, `mode`, `default_budget`, `cap(harness)`. Extensions as `Extension { harness, name }`.
- `src/cli/hook/payload.rs`: decoders for Claude, Codex, Grok, Gemini, Antigravity into `HandlerPayload`, `raw` kept. Release 1 registers Claude and Codex; the others decode for the tests and the later change.
- `src/cli/hook/run.rs`: the dispatcher above, spawning through `src/cli/process`.
- `src/cli/hook/encode.rs`: Claude and Codex encoders for every effect; the other harnesses' tables so an unsupported effect is reported.
- `src/cli/hook/adapters/`: built-in handlers that call the existing executables with their native payloads and translate to `Result`: `author-identity`, `dcg`, `git-ai`, `rtk`, `lint-on-write`, `session-capture`, `turn-checkpoint`, `tmux-status`, `jj-guards` (the worktree, agent isolation, and VCS internals parts as one handler). Each adapter declares `adopts`, the registration pattern of the entry it replaces.
- `src/cli/install/hooks.rs`: render one entry per subscribed event into Claude `settings.json` and Codex `hooks.json`; inventory the tables; adopt matching predecessor and legacy entries in Claude `settings.json` and Codex `hooks.json`, report a legacy entry in Codex `config.toml` with its line (that file also holds the trust state, so rune never rewrites it); write the manifest section; print the Codex trust step; `--check`.

## Data

- Handler payload: `{"v":1,"harness","event","native_event","session_id","turn_id","cwd","transcript_path","model","tool":{"name","id","input","response"},"prompt","stop_active","raw"}`.
- Result: `{"v":1,"effect":"pass|deny|continue_turn","reason","input","context":[]}`.
- Plan: `{harness: {native_event: {canonical, mode, budget_ms, handlers: [{id, exec, timeout_ms, on_failure, rewrites}]}}}`.
- Manifest `hooks` section per harness: `owned: [{event, registration_hash}]`, `foreign: [{event, source, command}]`, `adopted: [{event, source, command, by}]`.
- Identity: `user.name` and `user.email` in the checkout's `.jj/repo/config.toml`, and `git config user.name` / `user.email` when colocated.

## Ownership of the Claude table

The dotfiles render `~/.claude/settings.json` from a template. Release 1 turns that template into a chezmoi `modify_` script that takes the live file's `hooks` key as is, so rune's entries survive `chezmoi apply`, and declares no hook of its own. Legacy `sh ~/.sd/hook` entries are adopted by the first install, then the `dot_sd/hooks/` tree and the dispatcher go.

## Codex trust

Codex binds trust to the hook's hash. An entry rune writes is inactive until the owner accepts it in the harness (`/hooks`). The install prints that step, `--check` reports an untrusted rune entry, and the legacy Codex registration is adopted only once the rune entry is trusted, so no event goes unguarded in between.

## Order of work

1. Config, events, plan, `rune hook list`.
2. Dispatcher, decoders, encoders, modes, failure policy, rewriter order.
3. Install for Claude and Codex: ownership, adoption, manifest, trust step, `--check`.
4. The adapters and the identity handler; the deck hook fixture (a hook rune with frontmatter deploys as an executable and appears in the plan).
5. dotfiles retirement of `sd hook`; the live migration.
6. Proofs for every scenario; Astra and Grok on the code.

## Later changes

`harness-hook-dispatch-2`: Gemini `settings.json` (milliseconds), Grok `hooks/rune.json` with `[compat.claude] hooks = false`, Antigravity `hooks.json` with the event argument, the OpenCode shim plugin. `harness-hook-dispatch-3`: `rune doctor` reports hook drift, untrusted entries, and capture debt.

## Review dispositions

Astra on the specification (`gpt-6-astra`, clean state, 22 hits, 2026-09-27):

| Hit | Disposition |
|---|---|
| 1 the three jj guards vanish with the parts | fixed: `jj-guards` adapter, in the proposal and tasks |
| 2 Grok inherits Claude registrations | fixed: the dispatcher exits on Grok's environment in release 1, scenario added, record corrected |
| 3 a dispatcher error exits non-zero and the action proceeds | fixed: new requirement, strongest policy among subscribed handlers, never non-zero on a gate |
| 4 a guard approves an input a later rewriter changes | fixed: new requirement, rewriters before guards, effective input chained, `raw` kept |
| 5 Codex trust leaves the new entry inactive | fixed: trust step printed, `--check` reports, legacy adopted only after trust |
| 6 predecessor entries preserved as foreign | fixed: adoption requirement, adapters declare their pattern |
| 7 legacy registrations preserved by the modify script | fixed: adoption covers `sh ~/.sd/hook` in Claude settings and both Codex sources |
| 8 a spent budget never fails the pending guard | fixed: no-spawn failure by policy, scenario added |
| 9 the record's timeout consequence was wrong | fixed: caps per event in the requirement and the record |
| 10 a shared identity file collides across sessions | fixed: identity written into the checkout, own capability, scenario for two checkouts |
| 11 a stale listed identity after an unlisted model | fixed: unlisted writes an `unlisted@` identity the push gate refuses |
| 12 no consumer of the identity file | fixed: the checkout config is the consumer; scenario checks a commit |
| 13 Claude's model source unstated | fixed: the payload's `model`, which the live SessionStart script reads today; a fixture task |
| 14 conflict check on the command only | fixed: the full registration is hashed |
| 15 ownership by index | fixed: ownership by command prefix and event, foreign inventory by source |
| 16 `deny` on a passive event | fixed: plan validation error |
| 17 config re-read between install and run | fixed: the compiled plan is the only input to `run`; a newer config warns |
| 18 no schema or defaults | fixed: defaults and capabilities in the first requirement, plan shape in Data |
| 19 deck hook runes unconnected | fixed: task 4.4, a deck hook fixture in the plan; the rune shape itself is the deck's capability |
| 20 manifest lists only owned entries | fixed: foreign inventory required |
| 21 Grok decoder scoped out of release 1 | fixed: decoders for every harness in release 1, registration for Claude and Codex |
| 22 option 2 dismissed by an impossibility claim | fixed: the record states the cost |
