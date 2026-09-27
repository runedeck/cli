# Tasks

## 1. Config, events, plan

- [ ] 1.1 `src/cli/hook/config.rs`: `hooks:` parsed from the user config, defaults, capabilities, native extensions, validation that names the field (empty events, unknown event or capability, `deny` on a non-gate event, a rewriter after a guard)
- [ ] 1.2 `src/cli/hook/events.rs` and `plan.rs`: the eight canonical events, the per-harness native table with caps, modes and budgets, the compiled plan under the rune state directory; `rune hook list` and `--events`
- [ ] 1.3 Tests: the five-handler plan, an unknown event, an extension under one harness, a `deny` handler on `notification`, a session-end timeout clamped for Codex, a rewriter after a guard

## 2. Dispatcher

- [ ] 2.1 `src/cli/hook/payload.rs`: decoders for Claude, Codex, Grok, Gemini, Antigravity into the v1 payload, `raw` kept, event-name check against the argument
- [ ] 2.2 `src/cli/hook/run.rs`: `rune hook run --harness --native-event`, the plan as the only input, the Grok-inherited exit, serial handlers, gate, collect, passive, per-handler deadline from the budget, no-spawn failure, process-group kill, result parsing, failure policy, effective input chain, dispatcher failures by the strongest policy
- [ ] 2.3 `src/cli/hook/encode.rs`: Claude and Codex encoders for `pass`, `deny`, `continue_turn`, replacement input, context; unsupported effects on stderr
- [ ] 2.4 Tests: a Grok-shaped payload normalized, a denial encoded for Claude and for Codex, an event mismatch under a `deny` guard, a plan missing on a passive event, a guard timeout, a linter crash, a gate ending at the first denial, a collect merge in order, a spent budget, the rewritten command seen by the guard, the Grok-inherited call

## 3. Install

- [ ] 3.1 `src/cli/install/hooks.rs`: one entry per subscribed event into Claude `settings.json` and Codex `hooks.json`, ownership by command prefix and event, full-registration hash, foreign inventory, adoption of adapter predecessors and `sh ~/.sd/hook` entries in Claude settings and both Codex sources, the manifest section, the Codex trust step printed, `--check`
- [ ] 3.2 Tests: a cmux entry survives and is inventoried, one rune entry per event, a timeout edit stops the install, an unsupported event listed and skipped, git-ai adopted, the legacy Codex `config.toml` entry adopted, an untrusted Codex entry reported

## 4. Handlers

- [ ] 4.1 `author-identity`: harness and the payload's `model` against `authors.yaml` (checkout, else deck), identity written into the checkout's jj and git config, the `unlisted@` identity for an unknown model, a missing model warns. Fixture: a captured Claude `SessionStart` payload
- [ ] 4.2 Adapters over the existing executables and parts, each declaring what it adopts: dcg (`tool.before`, `deny`), git-ai checkpoint (`tool.before`, `tool.after`, `turn.finish`), rtk (`tool.before`, rewriter), lint-on-write (`tool.after`), session-capture (`session.end`, `compact.before`, detached), turn-checkpoint (`prompt.before`, `turn.finish`, `session.end`), tmux-status (`prompt.before`, `notification`, `turn.finish`, `session.end`), jj-guards (`tool.before`, `deny`: worktree, agent isolation, VCS internals)
- [ ] 4.3 Tests: the identity scenarios including two checkouts and a commit's author, each adapter's translation on a fixture payload, each guard's refusal
- [ ] 4.4 A deck hook rune fixture (frontmatter: event, handler, policy) deploys as an executable under the provider's `hooks/<deck>/` and appears in the plan through `.rune` selection

## 5. Retire sd hook

- [ ] 5.1 dotfiles: the Claude settings template becomes a modify script that keeps the live `hooks` block and declares no hook; `dot_sd/hooks/` and `dot_sd/executable_hook` removed after the first install adopted their entries; the dcg, git-ai, and rtk installers stop writing hook entries
- [ ] 5.2 Live: `rune install`, the Codex trust step taken by the owner, `--check` clean, one session of each harness confirms a commit authored by the model, a denied command, a lint finding, a captured session, and no duplicate checkpoint

## 6. Verification

- [ ] 6.1 Astra and Grok on the spec and record before code, dispositions in `design.md`
- [ ] 6.2 Astra and Grok on the code after group 4
- [ ] 6.3 `rune proof scaffold harness-hook-dispatch`, every scenario recorded, the cast page in the workshop, GIFs of a Claude session and a Codex session with the hooks live, the sign brief before the key touch
- [ ] 6.4 CLI-0020 amended, ASSEMBLY-0005 amended, CHANGELOG entries, issue 69 closed by the merge
