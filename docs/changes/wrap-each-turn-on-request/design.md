# Design

## Approach

One rule replaces attribution: a workspace that more than one session touched after the last wrap never wraps. The wrapper keeps one set per workspace, the keys of every session that showed itself there after the last wrap. Every rune hook event that places a session in the workspace adds its key: a prompt or a stop fired inside it, a tool event whose file path is inside it, or a shell command that names a path inside it, whatever the session's own directory. The built-in subscribes to all four events itself, so no config can leave one out. A payload with no session id adds the key `unknown`, a touch that belongs to nobody. Beside each key the set keeps the harness and the time of the first touch, and the card names a session by those two. A stop wraps only when the set holds that session's key alone, and the wrap empties the set. Any other set means no wrap, and the owner resets it by hand once the working copy is sorted out.

The set lives in one slot file under `.jj/`, read and written only under an exclusive lock on a file beside it, and replaced by a rename. The stop holds the lock from the check through the commit, so no prompt can register between them. A prompt that cannot take the lock in time leaves a marker file named for its key, and the next reader folds every marker into the set under the lock. The first touch writes its key before the jj snapshot, so a snapshot the dispatcher kills at its timeout loses the base and never the touch. Every failure fails toward no wrap.

The wrapper never blocks a turn, and never wraps one that may be unfinished. The dispatcher tells it which handlers before it failed or timed out, and any such handler means no wrap and one message. Every jj failure fails the handler under its policy, `warn` by default, and every skip is one message to the owner. The switch and the reset are files the owner changes, per workspace, and both refuse inside a model session.

## Runtime

```text
prompt.before, tool.before, tool.after (key = hash(harness, session id), or unknown)
    workspaces = switch on at cwd, plus at every path the tool wrote, plus at
                 every path a shell command names: absolute, relative to the
                 event's cwd, or ~ under HOME, as a word or a flag's value
    lock within 250 ms   -> fold markers, add key and its first touch, write
                            slot, then the first prompt's snapshot and a
                            second write
    lock late            -> write marker .jj/turn-wrap.touched/<key>
    slot unreadable      -> write marker (the stop reports the slot)

turn.finish (switch on at cwd, lock held from here to the end)
    dispatcher hold before it: the wrapper's timeout when this session is
                               alone, 500 ms for the card otherwise
    RUNE_MERGED_EFFECT is stop or deny  -> add key, set kept (the turn goes on)
    lock late (400 ms)                  -> marker, "another wrapper held <lock>"
    slot unreadable or wrong version    -> marker, "<why>; run rune hook wrap reset"
    add key; set is not {key}, or key is unknown
                                        -> "<who> touched the workspace", set kept
    RUNE_FAILED_HANDLERS is not empty   -> "<ids> failed or timed out", set kept
    @ empty                             -> remove slot, nothing
    @ described                         -> jj new            (turn joins the owner's change)
    @ bare                              -> jj commit -m "<harness>: turn <utc>"
    remove slot, card

rune hook wrap reset (owner, lock within 1 s)
    remove slot and markers, name the keys forgotten
```

The commit the first prompt records gives the card its stat on a described working copy. An empty working copy at the stop clears the set: there is nothing to attribute.

## Structure

- `src/cli/hook/wrap.rs`: `adapter_in`, `armed_in`, `armed_roots`, `named_paths`, `hold` with `Hold` and `CARD_HOLD`, `alone`, `open`, `register`, `touch`, `touch_of`, `finish`, `shared`, `label`, `unfinished`, `card`, `Lock`, `read_state`, the slot read and atomic write, `session_key`, `workspace_root`, `legacy_script`, `switch`, and `clear`.
- `src/cli/hook/adapters.rs`: `turn-wrap` in the built-in names and the dispatch table, `written_paths`, which names the file a tool event wrote, `notebook_path` included, and `shell_command`, which gives the command line of a shell tool.
- `src/hooks.rs`: `TURN_WRAP_EXEC`, `TURN_WRAP_TIMEOUT_MS` (1200), `TURN_WRAP_EVENTS`, `Handler::subscriptions`, which adds those four events to a wrapper's declared ones for validation, the policy, and `compile`, `PlannedHandler::wraps_turns`, and `settle`, which sorts the wrapper last on `turn.finish` whatever its order.
- `src/cli/hook/run.rs`: `RUNE_MERGED_EFFECT` and `RUNE_FAILED_HANDLERS` in every handler's environment, `wrap_hold` on `turn.finish` where the firing's workspace has the switch on and no legacy part covers the wrapper, capped at half the budget: the wrapper's timeout when an unlocked read shows this session alone, `CARD_HOLD` otherwise.
- `src/cli/hook/cover.rs`: the owner's legacy `~/.config/just/claude/turn/wrap` as the `turn-wrap` adapter's own command, so install recognizes the `Stop.d` part.
- `src/cli/surface/isolation.rs`: `model_session`, the first parent-session variable set, for the switch's refusal.
- `src/cli/mod.rs`: `HookAction::Wrap { On | Off | Status | Reset }`.

## Data

- Slot `.jj/turn-wrap.open`: `{"v":2,"keys":["claude-<16 hex>", ...],"commit":"<commit id>"?,"who":{"<key>":{"harness":"claude","since":"<UTC, seconds>"}}?}`, written to `turn-wrap.open.<pid>` and renamed. `commit` is the working copy's commit when the first key arrived. `who` keeps each key's harness and first touch, and a slot without it still reads.
- Lock `.jj/turn-wrap.lock`: an empty file, locked exclusively with `File::try_lock`, polled every 10 ms within the wait.
- Markers `.jj/turn-wrap.touched/<key>`: one file per key that could not take the lock, holding the key's harness and first touch as JSON. An empty marker still folds in, without a label.
- Card label: `a <harness> session (first touch <UTC>)`, `a session without an id` for `unknown`, and the key alone when the slot has no record for it.
- `CARD_HOLD`: the stop's lock wait of 400 ms plus an allowance of 100 ms for the read and the message. Nobody measured the allowance.
- Switch `.jj/turn-wrap`: the text `on`. Its presence is the switch.
- Session key: the harness name, a dash, and the first 16 hex digits of SHA-256 over `harness NUL session id`. No id gives the key `unknown`, which is never alone and so never wraps.
- `RUNE_FAILED_HANDLERS`: the ids of the handlers that failed or timed out before this one on the firing, comma-separated, empty when none did.
- Change description: `<harness>: turn <UTC, seconds>`.

## Findings closed

| finding | part that closes it |
|---|---|
| 1 another session's edits wrapped as this turn | the set: a prompt from any other session after the last wrap means no wrap for anyone until the owner resets. No attribution is read from jj |
| 5 re-read before commit not atomic with the mutation | the lock is held from the read of the set through the commit, so no prompt registers in between. A prompt that cannot take the lock leaves a marker that the next read folds in |
| probe as attribution (High) | there is no probe for attribution. The only jj reads are the commit id on the first prompt and `jj log -r @` on the stop, after the set check |
| displaced key (Medium) | no key is displaced. Every key stays in the set until a wrap empties it or the owner resets |
| `jj op revert` (Medium) | no revert. The wrap commits only when the set is this session alone, and the undo on the card is by content, `jj squash --from <wrapped> --into @ -u` |
| operation cap (Medium) | no operation log is read, so no cap |
| hold in every directory (High) | `wrap_hold` holds only when the firing's `cwd`, or `CLAUDE_PROJECT_DIR` without one, is in a workspace with the switch on, and at most half the event's budget |
| switch inside a model session | `switch` refuses `on`, `off`, and `reset` while a parent-session variable is set, with the code `hook.wrap_in_session` |
| legacy part unrecognized | `cover::runs` names the owner's `turn/wrap` script for `turn-wrap` on `turn.finish`, so install reports the part under the same-tool rule |
| 6, 7, 8 | as before: the wrapper last with `RUNE_MERGED_EFFECT`, the bounded hold, `jj new` on a described working copy |
| a session outside the set still wrapped (High) | every event that shows a session in the workspace registers it: a prompt or a stop fired there, or a tool event whose written path is inside it. A stop registers before its check, so a stranger's stop is in the set. No session id registers `unknown`, which blocks the wrap until the owner resets |
| the hold starved other Stop handlers, and a timed-out one still let the wrapper commit (High) | `wrap_hold` reserves time only when an unlocked read shows this session alone, and nothing when a legacy part covers the wrapper. The dispatcher passes `RUNE_FAILED_HANDLERS`, and the wrapper wraps nothing and says which handlers failed or timed out |
| the first prompt's key lost past the timeout (Medium) | `register` writes the key before the snapshot and the commit id in a second write |
| card minors | the bare card says its undo conflicts when a later turn changed the same file. The described card names `jj edit <change>` as its undo, which drops the empty working copy |
| a shell edit never registered (High) | `armed_roots` registers the session in every armed workspace a shell command names: an absolute path inside it, a `cd`, `-R`, `-C`, or `--repo` target that resolves inside it from the event's cwd, or any word that does. `written_paths` also reads `notebook_path` |
| a config could leave out a tool event (Medium) | `Handler::subscriptions` adds `prompt.before`, `tool.before`, `tool.after`, and `turn.finish` to any wrapper, so validation, the policy, and the plan see all four whatever the config declares |
| a slow stop handler hid the card (Medium) | `wrap_hold` reserves `CARD_HOLD` in an armed workspace another session touched, so the stop still reads the set and names the reset |
| the card named a hash (Minor) | the slot and the markers keep each key's harness and first touch, and `label` names a session by those |

## Limits

- A process that runs no rune hook is invisible. Its edits, and the owner's, carry no touch and wrap with the next turn of the one registered session. The card's stat shows the files.
- The owner-only switch is a convention. A session that runs the switch under `env -i`, or writes `.jj/turn-wrap` by hand, bypasses the refusal. Nothing in the wrapper can tell that write from the owner's.
- Codex shows no card on Stop. The owner there reads the result with `jj log`.
- A `Stop` hook outside rune can still commit mid-turn. The wrapper then wraps what is left.
- A denied prompt still marks the workspace touched. That errs toward no wrap.
- The switch belongs to one jj workspace root. A child `rune run` workspace does not inherit it.
- Two instances with one session id are one session.
- The owner's reset is the only way out of a shared set. The card says so.
- An edit whose shell command names no path in the workspace does not register: a script that writes there, a path a variable or a glob builds, or a relative chain the event's cwd cannot resolve. A command word also resolves under the event's cwd, which that cwd already registers. `~` resolves only through `HOME`.
- No harness puts a session name in its hook payload, so the card names a session by its harness and first touch.

## Coverage

| capability | requirement | check or open gap |
|---|---|---|
| wrapper-records-each-turn | The Owner Switches Wrapping Per Workspace | `tests/hook_wrap.rs`: round trip, `--json` from a subdirectory, refusal outside jj and inside a model session, off by default |
| wrapper-records-each-turn | A Turn Of One Session Becomes One Change | `tests/hook_wrap.rs`: described change, card, empty turn, every touch heard with only `turn.finish` declared. `wrap/tests.rs`: a tool event registers in the workspace its file is in, a shell command in every workspace it names, the paths a command names. `hooks.rs` tests: the wrapper's four subscriptions |
| wrapper-records-each-turn | A Workspace Two Sessions Touched Is Not Wrapped | `tests/hook_wrap.rs`: two sessions, a stranger's stop, a write and a shell command from another directory, a stop without a session id, reset, the card without a key. `wrap/tests.rs`: the shared message with labels, the unknown session |
| wrapper-records-each-turn | The Set Is Read And Written Under One Lock | `tests/hook_wrap.rs`: a prompt during a wrap held in `jj commit` by a shim, a lock held by the test, a malformed slot, a first snapshot past the timeout. `wrap/tests.rs`: lock, markers, open under a held lock, wrong version, the key before the snapshot |
| wrapper-records-each-turn | The Owner Resets The Set | `tests/hook_wrap.rs`: reset round trip, reset after a malformed slot |
| wrapper-records-each-turn | The Wrapper Runs Last And After The Final Stop | `dispatch_tests.rs`: order, merged effect, failed handlers, hold with the switch on and this session alone, the card hold with another session on record, none without the switch. `install.rs`: none under a legacy part. `tests/hook_wrap.rs`: continued turn, a failed stop handler, a slow stop handler in a shared workspace |
| wrapper-records-each-turn | The Card Keeps The Owner's Change | `tests/hook_wrap.rs`: described working copy and its undo. `wrap/tests.rs`: card text |
| wrapper-records-each-turn | The Legacy Part Is The Wrapper | `install.rs` tests: the owner's `20-turn-wrap.sh` reported as not first |
