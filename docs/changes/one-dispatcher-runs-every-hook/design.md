# Design

The research, the vendor protocol table, and Astra's schematics live in the workshop under `docs/specs/2026-09-27-rune-hooks/`. This file holds what the code needs.

## Runtime

```text
harness event, stdin JSON
        |
rune hook run --harness H --native-event N --plan PATH [--on-failure deny]
        |
if H is claude and the payload carries Grok's camelCase keys: exit 0, log, stop
read the compiled plan at PATH, the path install wrote into the registration
(a user config newer than the plan is a warning on stderr, never a reload)
plan missing or unreadable -> the registration's --on-failure decides: deny where H can block
check argument against the payload's event name
normalize -> HandlerPayload v1
        |
for handler in plan[H][event], serial, ordered:
    deadline = min(handler.timeout, remaining - reserve - 150 ms stop allowance)
    none left -> failure by policy, no spawn
    spawn exec, payload on stdin, RUNE_HARNESS RUNE_EVENT RUNE_HANDLER_ID RUNE_REMAINING_MS
    on expiry: SIGTERM, 10 ms grace, SIGKILL to the process group
    parse one JSON Result (unknown fields refused, v must be 1); empty stdout or anything
    else is failure by policy (warn: log; deny: Result deny)
    a replacement input from a `rewrites: true` handler becomes the effective input
    for the handlers after it; from any other handler it is a failure by policy
    continue_turn outside turn.finish is a failure by policy; a deny outranks every effect
    gate: stop on deny   collect: merge context in order, keep a deny   passive: ignore effects and input
        |
dispatcher failure (mismatch, decode, plan) -> strongest policy among subscribed handlers
encode Result for H and N -> stdout JSON, exit 0
```

## Modules

- `src/cli/hook/config.rs`: `HooksConfig { defaults, events, handlers }` from the `hooks:` key of the user config. `Handler { id, exec, events, order, timeout_ms, on_failure, requires, per_harness, rewrites, adopts }`. Validation names the field: empty events, unknown event or capability, `deny` on a non-gate event (a `per_harness` override included), a rewriter after a guard. The rewriter check runs per harness and native event on the policy each harness resolves, so a guard that is `deny` on one harness only orders the rewriters there.
- `src/cli/hook/plan.rs`: the compiled plan per harness and event, with clamped timeouts. `rune hook list` and `--events` print it.
- `src/cli/hook/events.rs`: `Canonical` (eight), `native(harness)`, `mode`, `default_budget`, `cap(harness)`. Extensions as `Extension { harness, name }`.
- `src/cli/hook/payload.rs`: decoders for Claude, Codex, Grok, Gemini, Antigravity into `HandlerPayload`, `raw` kept. Release 1 registers Claude and Codex; the others decode for the tests and the later change.
- `src/cli/hook/run.rs`: the dispatcher above, spawning through `src/cli/process`.
- `src/cli/hook/encode.rs`: Claude and Codex encoders for every effect; the other harnesses' tables so an unsupported effect is reported.
- `src/cli/hook/adapters/`: built-in handlers that call the existing executables with their native payloads and translate to `Result`: `author-identity`, `dcg`, `git-ai`, `rtk`, `lint-on-write`, `session-capture`, `turn-checkpoint`, `tmux-status`, `jj-guards` (the worktree, agent isolation, and VCS internals parts as one handler). Each adapter declares `adopts`, the registration pattern of the entry it replaces. The dcg adapter hands dcg every shell tool as one Claude `Bash` call: `Bash`, Gemini's `run_shell_command`, Antigravity's `run_command`, and Codex's `shell` (an argument list), `shell_command`, and `exec_command` (its `cmd` field). The [Codex hooks reference][CODEXHOOKS] matches `exec_command` as `Bash` (read 2026-10-24), so the other names are the shapes a decoded payload can still carry.
- `src/cli/install/hooks.rs`: render one entry per subscribed event into Claude `settings.json` and Codex `hooks.json`; inventory the tables; adopt matching predecessor and legacy entries in Claude `settings.json` and Codex `hooks.json`, report a legacy entry in Codex `config.toml` with its line (that file also holds the trust state, so rune never rewrites it); write the manifest section; print the Codex trust step; `--check`.

## Data

- Handler payload: `{"v":1,"harness","event","native_event","session_id","turn_id","cwd","transcript_path","model","tool":{"name","id","input","response"},"prompt","stop_active","raw"}`.
- Result: `{"v":1,"effect":"pass|deny|continue_turn","reason","input","context":[]}`.
- Plan: `{harness: {native_event: {canonical, mode, budget_ms, handlers: [{id, exec, timeout_ms, on_failure, rewrites}]}}}`.
- Plan generation: the plan narrowed to one harness and native event, in `hooks/plans/<sha256 of its text>.json` under the state directory.
- Manifest `hooks` section per harness: `owned: [{event, registration_hash}]`, `foreign: [{event, source, command}]`, `adopted: [{event, source, command, by}]`, `generations`, the generation paths the registrations name after the last install, `installs`, the count of installs that wrote the manifest, and `retired: [{generation, since, install, days, last_day}]`, each generation no table names with the time and the install count at which the tables stopped naming it, and the number of days on which installs found it unnamed.
- Identity: `user.name` and `user.email` in the checkout's `.jj/repo/config.toml`, and `git config user.name` / `user.email` when colocated.

## The registration

Each rendered entry is `rune hook run --harness H --native-event N --plan PATH`, plus `--on-failure deny` when a handler of that event resolves to `deny` and the harness can block it. `PATH` is the registration's own plan generation, made absolute against the install's working directory, because `RUNE_STATE_DIR` may be relative. A harness that starts the dispatcher elsewhere, without `RUNE_STATE_DIR`, or with another `XDG_STATE_HOME` then reads the plan install wrote. The policy in the registration is what a missing, truncated, or deleted generation falls back to. The dispatcher does not compile the config in that case, because the compiled plan stays its only source of handlers (review hit 17 below). A guarded Claude entry also carries `"onFailure": "block"`. The [Claude Code hooks reference][CCHOOKS] (read 2026-10-24) says the field arrived in v2.1.295 and that a failure then "does what exit code 2 does on that event". It covers a hook that cannot start, exits other than 0 or 2, times out, or prints invalid JSON, and it has no effect on `Stop`. The owner's Claude Code is 2.1.309. Codex documents no such field, so its entries carry none.

The stop allowance is 150 ms: the 25 ms poll that notices the deadline, the 10 ms grace, the group kill, and the drain of the pipes, each rounded up to the next poll. With the defaults a `tool.before` handler gets at most 2800 ms of the 3000 ms budget, and its denial leaves before Claude's 3 s timeout even when the handler ignores SIGTERM.

An entry is rune's when its command, read with shell quoting, starts with a binary named `rune` on any path, then `hook run`, and names the table's harness after `--harness`. So an install from a second build path finds the first one's entry. A known entry is replaced with the one this install renders, and a second known entry on the same event is removed, so each event keeps one dispatcher.

## The dispatcher log

`hooks/run.log` under the state directory gets one block per event: the time, the harness, the native event, `tool NAME`, and one `ID EFFECT` or `ID failed KIND` line per hook. The command, the file, and every reason stay out of it, because a command line can carry a credential and a guard often quotes the line it refused. The hook record holds the rest without text. The file is created with mode 0600, and at 256 KiB it moves to `run.log.1`, so at most two generations remain.

## Install as one step

`install::prepare` reads the manifest and every registered harness table and computes the new tables without writing. A hand-edited owned entry or an unreadable table stops it there. The new plan generations are written first, then `publish` stages each changed table and the manifest beside their targets and renames them in that order. Each staged file has a name no other run uses, from the process id, the clock, and a counter, and it is created new, so two runs never write one file. A staged file whose write or permission change fails is removed at once, so no partial file stays beside its target. Before a rename, the old target is kept beside it as a hard link, or a copy where the file system has no links. When a rename fails, each target already replaced gets its backup back by rename, so a read-only target goes back too, and a target that did not exist is removed again. A restore that fails is named in the error. An install that writes holds an exclusive lock on the directory of every file it may write, the manifest's and each harness table's, from the first read to the last generation deleted, so a second install waits for it. The generations live under the manifest's directory, so its lock covers them. The lock is std's `File::lock` on an empty `.rune-install.lock` in each directory: `flock` on Unix and `LockFileEx` on Windows, so it holds on every platform rune is released for. A `flock` on the directory itself left nothing behind, but Windows opens no directory as a file, and the Windows build took no lock at all (found in review, 2026-10-24). It creates each of those directories before it locks them, so every install that shares a target locks the same directory. Locking the nearest existing ancestor instead let a second install lock the directory the first one created while staging, and both held their locks at once (found in review, 2026-10-24). Each directory is locked once, by its canonical path. A check or a dry run writes nothing and takes no lock. A relative `RUNE_STATE_DIR` once named the state directory both relatively and absolutely, and the second lock waited for the first forever (found 2026-10-24).

A config with no handler still reconciles when an earlier install left a plan generation or an owned entry in the manifest. Install then renders an empty plan: rune's commands go, every foreign command stays, and every generation is recorded as no longer named. With neither, there is nothing to take back, and install writes nothing.

A proof scene keeps the runner's HOME for tools that need it, such as a harness login, so a hook install from a scene would point the live registrations at a plan the runner deletes. The runner writes a marker file into each scene's capture directory, with its own process id and HOME, and names it in `RUNE_PROOF_SCENE`. The capture directory goes when the scene ends, and the marker goes with it. A hook install counts the marker only while the file exists and the process it names still runs, so a shell that carries the variable after the proof installs as usual. With a live marker for its HOME, `rune hook install` fails and names the HOME, and `rune install` reports the hook step as `skipped` with the reason. The check comes before the config is read, so the skip line is the same on every machine.

A table that is a symbolic link, such as a `settings.json` the dotfiles keep, is resolved first, and the file it points to is the one staged and replaced, so the link survives. A replaced file keeps its permission bits, and a new table is created with mode 0600. `table.rs` reads the table's members in file order with `serde_json`'s `RawValue` and writes back only the `hooks` member. Every other member keeps its text and place, and so does every event under `hooks` the install left alone. `serde_json`'s `preserve_order` feature would keep the order too, but it applies to the whole crate. It lets `HashMap` iteration order into every `serde_json::Value` rune prints, so `rune config reference` changed from one run to the next (tried 2026-10-24). Registration hashes are taken over the entry with its keys sorted, so a key order change alone is no conflict.

## Plan generations

Each registration names its own plan generation: the compiled plan narrowed to the registration's harness and native event, in `hooks/plans/<digest>.json`, where the digest is the SHA-256 of the file's text. No install rewrites a generation. An install writes the generations of its registrations before the tables, so a table never names a file that is not there. A registration of the old tables reads the plan it was installed with, byte for byte, while its table changes and for as long as the generation stays. The dispatcher reads a generation only when its bytes hash to the digest in its name. An edit that clears a guard's handlers still leaves a valid plan, and the event would run unguarded, so a mismatch is a plan that does not load, and the registration's `--on-failure` policy answers. `--check` names such a file, and the next install writes it again. An event whose plan did not change keeps its generation, its command, and so its Codex trust, which covers the command. A generation of the whole plan would change every registration's command on every config edit, and Codex would stop running each one until the owner trusted it again.

A generation outlives the tables that named it. The owner runs many Claude and Codex sessions at once, and a session that loaded its hooks before an install keeps calling the old `--plan` path until it restarts. Under `deny`, each guarded call of that session would fail if the file were gone. So the manifest's `retired` list records, for each generation no rune registration names, the time and the install count at which the tables stopped naming it, and the days on which installs found it unnamed. `retire.rs` holds the rules. The manifest is the last file of the step, so an install killed after the tables changed leaves the old manifest. So, before the tables change, install writes the old manifest once more without the record of any generation the old or the new tables name. A record that survives a kill is then never older than the last time a table named its generation, and a kill can only delay a deletion (found in review, 2026-10-24). The step's own manifest drops the same records and gives each generation the old tables name and the new ones do not a fresh record at this install. After publication, install reads every table again, records any other unnamed generation, and counts today for each record once. It deletes a generation only when its record is 14 days old, at least two more installs have run, and installs on 14 different days have found it unnamed. The recorded time decides over the mtime, because a reused generation's mtime moves with each install that names it. A clock set forward ages a record, and quick installs raise the count, but neither adds a day on which an install ran, so the day count bounds the window by real days. The review offered a second rule, 14 days from the later of the record and the last install. The day count is simpler, because it needs no time beyond the record's own, and it still lets a generation go for an owner who installs every day. A generation holds the handlers of one event, so the window costs little disk. An install that fails stops before the cleanup, so its new generations stay unrecorded until the next install records them.

`rune hook install --check` reports a registration whose generation is missing, and the dispatcher answers such a registration with its `--on-failure` policy. That is the limit of the window: a session that runs past the 14 days, two more installs, and 14 install days after the tables stopped naming its generation falls to that policy, a guarded event then denies every call, and a restart loads the current registrations.

The plan used to be one file, and the install published a bridge before the tables: the new plan merged with the old. Review found three ways the merge failed an old guard (2026-10-24). An observer that reused a guard's id replaced the guard. A new rewriter could run after an old guard and hand out an unchecked input. A new passive mode turned the old guard's denial into a log line. Generations remove the merge, so all three are gone.

`rune hook list --write` still writes the whole plan to `hooks/plan.json` for a machine that installs nothing, and `rune hook run` reads that file when no `--plan` names another.

## Ownership of the Claude table

The dotfiles render `~/.claude/settings.json` from a template. Release 1 turns that template into a chezmoi `modify_` script that takes the live file's `hooks` key as is, so rune's entries survive `chezmoi apply`, and declares no hook of its own. Legacy `sh ~/.sd/hook` entries are adopted by the first install, then the `dot_sd/hooks/` tree and the dispatcher go.

Install adopts a predecessor only from a command that runs it alone. It reads the command's words with shell quoting. The legacy shapes are `[sh|bash|zsh] PATH/.sd/hook EVENT`, its successor `[sh|bash|zsh] PATH/.config/just/hook EVENT`, and `sd hook EVENT`, and a tool is the program's file name. The turn-checkpoint adapter runs `~/.config/just/claude/turn/checkpoint`, else `~/.sd/claude/turn/checkpoint` for a home the dotfiles have not moved yet. A command with `;`, `&`, `|`, `<`, `>`, `(`, `)`, `$`, a backtick, or a line break joins, redirects, or substitutes commands. Such a command, for example `sh ~/.sd/hook PreToolUse && /owner/check-policy`, stays whole. Install warns that it runs the predecessor with other commands, and the owner splits it by hand.

## Codex trust

Codex binds trust to the hook's hash. An entry rune writes is inactive until the owner accepts it in the harness (`/hooks`). The install prints that step, `--check` reports an untrusted rune entry, and the legacy Codex registration is adopted only once the rune entry is trusted, so no event goes unguarded in between.

The same rule holds for rune's own earlier entry. A changed plan gives an event a new generation, so its entry gets a new command and a new hash, and Codex skips it until the owner trusts it. Replacing the trusted entry in place left the event unguarded until then (found in review, 2026-10-24). Install now keeps an earlier rune entry that Codex trusts where it stands, records it as owned again, and adds the new entry in a group of its own after it. The trust step names the new entry's position, and `--check` fails until Codex trusts it there. The next install then removes the earlier entry and leaves its group as `{"hooks": []}`, so the new entry keeps the position Codex trusts. Until that install, a trusted pair runs the event's handlers twice, from both generations.

A removal moves every later hook of its group to a lower handler, and Codex keys trust by handler too. Removing rune's earlier entry from a group it shared with a guard Codex trusts moved the guard, and Codex skipped it while `--check` passed (found in review, 2026-10-24). On Codex, install now tries each event's removals first: rune's earlier entry, a duplicate, an entry the plan drops, and an adopted predecessor. It holds every removal that would move a hook Codex trusts where it stands, until no removal moves one. The output names the held hook, the hook it keeps in place, and the step: remove the held hook by hand, then trust the moved hook again in `/hooks`. `--check` reports each held removal, and every hook that is not rune's and that Codex does not trust where it stands, in a group that holds a rune entry or loses a hook. A hook the owner disabled in Codex is the owner's choice, and is no drift.

`codex_trust.rs` checks that trust the way Codex does. The [Codex hooks reference][CODEXHOOKS] says Codex records trust against the hook's current hash and runs no untrusted non-managed hook. Codex keeps it in the user `config.toml` as `[hooks.state."<hooks.json path>:<event label>:<group>:<handler>"] trusted_hash`. The hash is `sha256:` over the sorted JSON of `event_name`, the group's `matcher`, and the one handler's `type`, `command`, `timeout`, and `async` (`hook_hash` in [Codex's discovery code][CODEXDISCOVERY], read 2026-10-24). A recomputation matched the hash Codex stored for the owner's dcg entry. Install reconciles each guarded event twice before it writes: once with every predecessor kept and once with every predecessor removed. It reads the rune entry's trust at its position in the second table, the table that adoption would write. Only an entry Codex trusts there lets the predecessors go. A Codex group that adoption or removal empties stays as `{"hooks": []}` while a later group follows, rune's new group included, because Codex keys trust by position ([codex#49399][CODEX49399]). A predecessor before the rune entry in the same group still moves it to the next lower handler number. Install then keeps every predecessor, says which position the entry would move to, and asks the owner to remove the predecessor by hand and trust the entry again. A predecessor that shares a group with other hooks still moves the hooks after it, and their trust has to be renewed.

The Codex encoder sends `permissionDecision: "allow"` beside `updatedInput`. The Codex hooks reference says "Return `updatedInput` only with `permissionDecision: "allow"`", so without it the rewrite is reported as an error and dropped. The reference does not say whether that `allow` also skips Codex's own approval for the call. The encoder keeps it. The only rewriter is rtk, which the owner keeps in every mode, and the plan validator puts every rewriter before the guards, so the guards judge the rewritten call.

## Order of work

1. Config, events, plan, `rune hook list`.
2. Dispatcher, decoders, encoders, modes, failure policy, rewriter order.
3. Install for Claude and Codex: ownership, adoption, manifest, trust step, `--check`.
4. The adapters and the identity handler; the deck hook fixture (a hook rune with frontmatter deploys as an executable and appears in the plan).
5. dotfiles retirement of `sd hook`; the live migration.
6. Proofs for every scenario; Astra and Grok on the code.

## Later changes

`one-dispatcher-runs-every-hook-2`: Gemini `settings.json` (milliseconds), Grok `hooks/rune.json` with `[compat.claude] hooks = false`, Antigravity `hooks.json` with the event argument, the OpenCode shim plugin. `one-dispatcher-runs-every-hook-3`: `rune doctor` reports hook drift, untrusted entries, and capture debt.

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

[CCHOOKS]: https://code.claude.com/docs/en/hooks
[CODEXHOOKS]: https://developers.openai.com/codex/hooks
[CODEXDISCOVERY]: https://github.com/openai/codex/blob/main/codex-rs/hooks/src/engine/discovery.rs
[CODEX49399]: https://github.com/openai/codex/issues/49399
