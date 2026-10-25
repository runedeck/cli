# Tasks

## 1. Config, events, plan

- [x] 1.1 `src/hooks.rs`: `hooks:` parsed from the user config, defaults, capabilities, native extensions, validation that names the field (empty events, unknown event or capability, `deny` on a non-gate event, a rewriter after a guard)
- [x] 1.2 `src/hooks.rs` and `src/cli/hook/plan.rs`: the eight canonical events, the per-harness native table with caps, modes and budgets, the compiled plan under the rune state directory; `rune hook list` and `--events`
- [x] 1.3 Tests: the five-handler plan, an unknown event, an extension under one harness, a `deny` handler on `notification`, a session-end timeout clamped for Codex, a rewriter after a guard

## 2. Dispatcher

- [x] 2.1 `src/cli/hook/payload.rs`: decoders for Claude, Codex, Grok, Gemini, Antigravity into the v1 payload, `raw` kept, event-name check against the argument
- [x] 2.2 `src/cli/hook/run.rs`: `rune hook run --harness --native-event`, the plan as the only input, the Grok-inherited exit, serial handlers, gate, collect, passive, per-handler deadline from the budget, no-spawn failure, process-group kill, result parsing, failure policy, effective input chain, dispatcher failures by the strongest policy
- [x] 2.3 `src/cli/hook/encode.rs`: Claude and Codex encoders for `pass`, `deny`, `continue_turn`, replacement input, context; unsupported effects on stderr
- [x] 2.4 Tests: a Grok-shaped payload normalized, a denial encoded for Claude and for Codex, an event mismatch under a `deny` guard, a plan missing on a passive event, a guard timeout, a linter crash, a gate ending at the first denial, a collect merge in order, a spent budget, the rewritten command seen by the guard, the Grok-inherited call

## 3. Install

- [x] 3.1 `src/cli/hook/install.rs`: one entry per subscribed event into Claude `settings.json` and Codex `hooks.json`, ownership by command prefix and event, full-registration hash, foreign inventory, adoption of adapter predecessors in Claude settings and both Codex sources, the manifest section, the Codex trust step printed, `--check`
- [x] 3.2 Tests: a cmux entry survives and is inventoried, one rune entry per event, a timeout edit stops the install, an unsupported event listed and skipped, git-ai adopted, a tool's Codex `config.toml` entry named with its line, an untrusted Codex entry reported

## 4. Handlers

- [x] 4.1 `author-identity`: harness and the payload's `model` against `authors.yaml` (checkout, else deck), identity written into the session's jj workspace config and git worktree config, the `unlisted@` identity for an unknown model, a missing model warns. Fixture: a captured Claude `SessionStart` payload
- [x] 4.2 Adapters over the existing executables and parts, each declaring what it adopts: dcg (`tool.before`, `deny`), git-ai checkpoint (`tool.before`, `tool.after`, `turn.finish`), rtk (`tool.before`, rewriter), lint-on-write (`tool.after`), session-capture (`session.end`, `compact.before`, detached), turn-checkpoint (`prompt.before`, `turn.finish`, `session.end`), tmux-status (`prompt.before`, `notification`, `turn.finish`, `session.end`), jj-guards (`tool.before`, `deny`: worktree, agent isolation, VCS internals)
- [x] 4.3 Tests: the identity scenarios including two checkouts, two workspaces of one repository, the push gate on the unlisted identity, and a commit's author, each adapter's translation on a fixture payload, each guard's refusal
- [ ] 4.4 (deferred to the deck change that gives hook runes their frontmatter; until then a deck hook is declared as any other `exec` handler) A deck hook rune fixture (frontmatter: event, handler, policy) deploys as an executable under the provider's `hooks/<deck>/` and appears in the plan through `.rune` selection

## 5. Dotfiles and the live migration

- [ ] 5.1 dotfiles: the Claude settings template becomes a modify script that keeps the live `hooks` block and declares no hook; the dispatcher and its parts stay under `~/.config/just/`; the dcg, git-ai, and rtk installers stop writing hook entries
- [ ] 5.2 Live: `rune install`, the Codex trust step taken by the owner, `--check` clean, one session of each harness confirms a commit authored by the model, a denied command, a lint finding, a captured session, and no duplicate checkpoint

## 6. Verification

- [x] 6.1 Astra on the spec and record before code, dispositions in `design.md`; Grok pending the proxy route in its profile
- [ ] 6.2 Astra and Grok on the code after group 4
- [ ] 6.3 `rune proof scaffold one-dispatcher-runs-every-hook`, every scenario recorded, the cast page in the workshop, GIFs of a Claude session and a Codex session with the hooks live, the sign brief before the key touch
- [ ] 6.4 CLI-0020 amended, ASSEMBLY-0005 amended, CHANGELOG entries, issue 69 closed by the merge

## 7. Review fixes, round two

- [x] 7.1 The registration: the plan and the manifest by absolute paths, a guarded registration that denies when its plan lacks the event, and a bridge plan with the events of both installs while the tables change
- [x] 7.2 Install: a failed hook step fails `rune install`, old files kept and restored by rename with a failed restore named, unique staging names, and one lock per directory by its canonical path
- [x] 7.3 Install: an empty handler list takes rune's entries back, Codex trust read where adoption puts the entry, a predecessor adopted only from a command that runs it alone, and no hook installed from a proof scene with the runner's HOME
- [x] 7.4 Tests: every integration test runs rune with a scratch HOME, config, and state, and a guard test fails when one sees the real HOME
- [x] 7.5 Proof: every scenario of the closure recorded with `rune proof run`, 49 of 49, `guards-hold-under-failure` and `install-respects-owner-tables` included

## 8. Review fixes, round three

- [x] 8.1 Proof scenes: a marker file in the runner's capture directory with its process id and HOME, counted only while the runner runs, and `rune install` reports the hook step as `skipped` with the reason
- [x] 8.2 Install: each target directory created before it is locked, the bridge keeping every old handler of a shared event, and the bridge and the final plan in one transaction
- [x] 8.3 Tests: every test starts rune through the support module, and the guard test checks each line that could start it or clear its environment
- [x] 8.4 Proof: the rollback scene fails during the replacement and checks the error and every restored file, and the marker scenes are recorded

## 9. Review fixes, round four

- [x] 9.1 Install: each registration names its own plan generation, written before the tables and never rewritten, and the bridge and its two-step transaction are gone
- [x] 9.2 Install: after the tables and the manifest, install reads the tables again to find the generations no table names, and the manifest lists the generations the registrations name
- [x] 9.3 Tests: an old registration keeps its plan when an observer reuses a guard's id, a rewriter runs after a guard, or the event turns passive
- [x] 9.4 Proof: scenes for a failed and a successful install over an old registration, and the scenes that read the single plan file read the generations
- [x] 9.5 Install: a generation no table names stays 14 days from the time the manifest records, so a session that loaded the old hooks keeps its plan, and a failed install writes nothing after its generations

## 10. Review fixes, round five

- [x] 10.1 Codex: a changed plan keeps rune's earlier entry that Codex trusts, adds the new one in a group of its own, and fails `--check` until Codex trusts it, and the next install then removes the earlier one
- [x] 10.2 Dispatcher: a generation whose bytes do not hash to the digest in its name does not load, so the registration's failure policy answers, and `--check` names the file
- [x] 10.3 Install: the retired records are reconciled before publication, a generation goes only after 14 days and two more installs, and the manifest counts its installs
- [x] 10.4 Install: the lock is std's file lock on a file in each directory, so it holds on Windows too

## 11. Review fixes, round six

- [x] 11.1 Install: before the tables change, the old manifest drops the record of every generation the old or the new tables name, so a killed install can only delay a deletion
- [x] 11.2 Codex: a removal that would move a hook Codex trusts waits, the output names the step, and `--check` checks every hook in the groups install touches
- [x] 11.3 Install: a generation goes only after installs on 14 different days have found it unnamed, so a clock set forward cannot end the window
- [x] 11.4 Install: a staged file whose write fails is removed at once

## 12. The legacy dispatcher keeps its entry

- [x] 12.1 `src/cli/hook/legacy.rs`: the dispatcher shapes, the known script content, the parts in name order up to the first that is not a regular file, each part's shape (an optional `-x` or `-f` guard on the program of its one `exec` line), `$HOME` expanded outside single quotes, a bare `sh` or `/bin/sh` with no option that resolves to `/bin/sh`
- [x] 12.2 Install: a legacy dispatcher entry stays and is never advised away, every handler stays in the generation, and each handler whose whole command a part runs is recorded in `hooks/cover/<harness>-<event>.json` with `covered:` naming both. Nothing is covered for a `deny` handler, a handler with arguments, a reshaped payload off Claude, a matcher other than `*`, or a compound line, and the warning says where a part may run twice. No record is written for Codex. Codex `config.toml` hooks are read with the TOML parser and named by their real line. A gone entry an earlier install adopted is named
- [x] 12.3 `src/cli/hook/cover.rs` and `rune hook run`: the cover read at each firing, the handler skipped only while the entry, its matcher, the shell, the script, the part, its guard, and the command still hold, and the hook record carries `skipped` with the part
- [x] 12.4 Tests: the owner's `Stop.d` covers tmux-status and turn-checkpoint on Claude with the entry kept, every handler in the generation, both parts named, and the cover staged. The dispatcher skips both, runs tmux once its part is gone and turn once its guard fails, with no install and no registration change. The `PreToolUse.d` guards keep running beside dcg and jj-guards. A dcg part never covers the `deny` handler. An earlier part without the shape or with a `-e` guard stops the count. A matcher or a compound line covers nothing. A Codex part covers nothing, and a Codex record is refused at the firing. A gone adopted entry is named and its record kept until the entry is back

## 13. A doubt runs the handler

- [x] 13.1 `src/cli/hook/legacy.rs`: a counted line is printable ASCII with no `\r`, backslash, or open quote, split on space and tab, in a file with plain `\n` endings. The shell in front is `sh` or `/bin/sh` and resolves by canonical path to `/bin/sh`, the `sh` on `PATH` that runs each part resolves there too, and a direct entry needs an executable script whose first line is exactly `#!/bin/sh`
- [x] 13.2 `src/cli/hook/legacy.rs` and install: a folder counts only when every `*.sh` entry is a regular file in the shape, so the order of the parts cannot change the result
- [x] 13.3 `src/cli/hook/cover.rs`: a hook or group with a key rune does not read covers nothing. `holds` reads the table under `CLAUDE_CONFIG_DIR`, takes `HOME` from the environment alone, checks the record's harness and event, refuses every record for Codex, and matches the handler about to run by command, policy, and adapter command
- [x] 13.4 `src/cli/hook/run.rs` and `legacy.rs`: every cover file is a regular file under a limit, read without blocking. The budget clock starts before the cover read, a cover that holds past the deadline still skips its handler with the part recorded, and one firing reads each table, the script, each folder, and each shell word once
- [x] 13.5 Install records each slot from the table it writes and names a Codex `config.toml` hook by the line of its command value. The capability spec is under 150 lines, each requirement under 100 words, each step under 30
- [x] 13.6 Tests: a CRLF or non-ASCII part, a fake `sh` first on `PATH`, a shebang with an option, a `nohup` part beside a good one, an `if` key at install and at a firing, `HOME` unset or another home, `CLAUDE_CONFIG_DIR` elsewhere, a record copied to another harness, a changed handler, a FIFO table, a cover that holds past the deadline, the cache, the slot, and the TOML line

## 14. Nothing on Codex, only the system shell, and a bounded scan

- [x] 14.1 Codex: install writes no Codex cover record and prints no `covered:` or trust line for it, `holds` refuses every Codex record, and the direct-entry handover keeps its trust check
- [x] 14.2 `src/cli/hook/legacy.rs`: only `sh` or `/bin/sh` in front, resolved on the firing's `PATH` by canonical path to `/bin/sh`, the inner `sh` resolved the same way at install and at each firing, and a shebang counted only as `#!/bin/sh`
- [x] 14.3 `src/cli/hook/legacy.rs`: a folder counts at most 16 `*.sh` entries, one scan reads at most 256 KiB of parts, and past a cap the folder covers nothing and install says why
- [x] 14.4 `src/cli/hook/legacy.rs`: a counted word with an unquoted `*`, `?`, `{`, or a `[` paired with `]` covers nothing, and `[` alone stays the test command of a guard
- [x] 14.5 `src/cli/hook/run.rs`: a cover that holds past the budget records `skipped` with its part, and the comment says so
- [x] 14.6 Tests: a Codex entry covers nothing at install and at the firing, `zsh` and `bash` entries cover nothing, a fake `sh` first on `PATH` at install and at a firing, a shebang other than `#!/bin/sh`, seventeen parts, parts past the byte cap, an unquoted bracket pair, and a cover that holds past the budget

## 15. Only the first part, a quiet environment, and no rewriter

- [x] 15.1 `src/cli/hook/legacy.rs`: only the first `*.sh` part can cover, because the dispatcher stops at the first part that fails. The first part is decided by name under every locale: `NN-name.sh` with no two names on one number. The first part must be a regular file in the shape, a later part is not read, and every directory entry counts toward the cap of 16
- [x] 15.2 `src/cli/hook/legacy.rs` and `cover.rs`: a counted word expands `$HOME` and `${HOME}` only, so any other `$`, a backquote, or a leading `~` covers nothing. Install and the firing cover nothing while `BASHOPTS`, `BASH_COMPAT`, `BASH_ENV`, `ENV`, `EXECIGNORE`, `GLOBIGNORE`, `GLOBSORT`, `IFS`, `POSIXLY_CORRECT`, `SHELLOPTS`, or a `BASH_FUNC_*` name is set, and install names the variable
- [x] 15.3 `src/cli/hook/install.rs` and `cover.rs`: a handler with `rewrites: true` is never covered. `[ -x ]` is read with `access(2)`. `words` splits a predecessor on any whitespace as on main, and a part on space and tab only
- [x] 15.4 Design: a doubt runs the handler and a cover that holds skips past the deadline. Rune keeps every generation because the Claude Code docs say edits are "normally" picked up by a file watcher and rune cannot read when a session last loaded its table. The shell variable list with its bash 3.2 and 5.3 manual sources. A Codex `config.toml` over 8 MiB or not a regular file reads as untrusted, so the handover keeps the predecessor
- [x] 15.5 Tests: the owner's `Stop` parts cover tmux alone and turn runs twice, a `nohup` part after the first part blocks nothing, a first part outside the shape or a directory first is named, `11-A.sh` and two parts on one number cover nothing, a rewriter with a matching part, `SHELLOPTS` at install and `BASH_FUNC_ls%%` at a firing, and `$OTHER` in a part path at install and at a firing

## 16. A passive event keeps what a handler says, and install names every gap

- [x] 16.1 `src/cli/hook/run.rs`: `merge` keeps the context and the message of every handler on a passive event, each message line named for its handler, and ignores the effect and the input there
- [x] 16.2 `src/cli/hook/encode.rs`: a `message` field in the result. Context and a message go into the field each harness's reference documents for that event, and each is reported on stderr as `reported, not applied` where the event has none. The Claude Code and Gemini CLI references were read on 2026-10-25, the Codex reference on 2026-10-24
- [x] 16.3 `src/cli/hook/install.rs`: an event a harness lacks is reported for every harness the plan targets, a harness install registers nothing for included
- [x] 16.4 `src/cli/hook/install.rs` and `legacy.rs`: the parts behind the first are read for one message alone, from a budget of their own, and each handler whose command a later part runs is named with the part and the reason, and covered by nothing
- [x] 16.5 Tests: context on Claude `SessionStart` reaches `additionalContext`, two handlers' messages reach `systemMessage` in order on Claude `SessionStart` and Gemini `SessionEnd`, Claude `Notification` reports both as not applied, a deny on `SessionStart` is logged while its context stays, Antigravity and OpenCode `session.end` are named unsupported, and the owner's `Stop` parts name turn by its third part

## 17. The commit, the shared config, and the native cap

- [x] 17.1 `src/cli/hook/adapters.rs`: after the jj config write, `jj metaedit --update-author -r @` on the workspace's own undescribed working-copy commit, a described one left alone and named in the context, and no other workspace's commit touched
- [x] 17.2 `src/cli/hook/adapters.rs`: with `core.bare` or `core.worktree` in the repository config, no git identity is written, and the context names the key and says a raw git commit carries the global git identity and why
- [x] 17.3 `src/cli/hook/adapters.rs`: `extensions.worktreeConfig` read from the repository config alone, and turned on only when no `config.worktree`, the main worktree's or one under `worktrees/`, sets `core.bare` or `core.worktree` or cannot be read, else no git identity and the file named
- [x] 17.4 `src/hooks.rs`: a handler subscribed by a native name resolves through `Canonical::from_native` and takes that harness's cap, so `codex:SessionEnd` at 3001 ms renders 3 s and `claude:SessionEnd` at 60001 ms renders 60 s
- [x] 17.5 Tests: the working-copy author in one and two workspaces, a described commit kept and named, `core.worktree` with no git identity and the note, a dormant `config.worktree` under the main and the linked worktree with the extension off and `git status` alive, a global extension not taken for the repository, and the native cap on every harness and native event

## 18. A passive event steers through no field, and a line cannot fake a name

- [x] 18.1 `src/cli/hook/encode.rs`: `encode` takes the event's mode. On a passive event it drops a denial and a replacement input, withholds `additionalContext` on Claude `Stop`, where the reference reads it as a continuation, and on Grok `Stop`, which no reference in the repository describes, and reports each on stderr as `reported, not applied`
- [x] 18.2 `src/cli/hook/encode.rs`: Gemini's `systemMessage` on `BeforeAgent`, `BeforeTool`, `AfterTool`, and `AfterAgent`, because the reference lists it as a common output field. Only `BeforeToolSelection`, which the dispatcher does not map, refuses it
- [x] 18.3 `src/cli/hook/run.rs`: `visible` shows a carriage return as `\r`, a newline in a message line as `\n`, and every other C0, C1, and delete control as `\xNN`, keeping tab and a context newline. `merge` applies it to each message line before the handler's name and to each context line
- [x] 18.4 Tests: a passive `Stop` on Claude carries the message and no context, with the stderr note, and continues the turn without the override, a passive encode drops a deny and an input on Claude, Gemini, Codex, and Grok, Gemini `BeforeTool` carries the message, a carriage return, a backspace, a cursor escape, a C1 control, and a title escape each fail to fake `guard:` with a tab kept, and every C0, C1, and delete code goes through `visible`

## 19. The commit that is the session's own, the included config, and the hidden character

- [x] 19.1 `src/cli/hook/adapters.rs`: `jj metaedit --update-author -r @` only when `jj log -r @` says the working-copy commit is empty, undescribed, on one parent, mutable, and the working copy of this workspace alone, by `working_copies`; else `jj new`, so the session starts in a fresh working copy and the earlier commit keeps its author. The context says which happened and names the commit
- [x] 19.2 `src/cli/hook/adapters.rs`: each `config.worktree` read with `--includes --show-origin --name-only`, so a `core.bare` or `core.worktree` behind an `include.path` keeps the extension off and the context names the included file; a `config.worktree` that uses `includeIf` or cannot be read with its includes keeps it off too
- [x] 19.3 `src/cli/hook/run.rs` and `encode.rs`: `visible` shows every Unicode format character (Cf) and bidi control as `\u{NNNN}`, and `encode` passes the reason through it before any harness reads it
- [x] 19.4 Tests: an empty working copy takes the author, one with edits, a merge, a described one, one shared with a second workspace through `jj edit`, and an immutable one each keep their author with a new working copy on top and the reason named, an unlisted model's commit fails `scripts/check-authorship`, a `core.bare` behind an include, a malformed included file, and an `includeIf` each keep the extension off with `git status` alive in the linked worktree, the bidi override, isolates, zero-width characters, marks, and byte-order mark fail to fake `guard:` in a message and in context, a deny reason reaches Claude and Gemini with each shown, a dispatcher failure's reason too, and every listed format character goes through `visible`
