# rune — tour and manual review plan

## What rune is

rune deploys **markdown instruction files** (skills, agents, rules, hooks) into AI coding
harnesses (`.claude/`, `.codex/`, `.gemini/`, `.opencode/`). You author content once in a
**deck**; each consumer project picks what it wants and rune assembles, transforms, and
deploys it, tracking exactly what landed so it can detect drift and clean up.

## Vocabulary (one word each)

| Term | Meaning |
|---|---|
| **rune** | one instruction file — a skill, an agent, a rule, or a hook |
| **deck** | a collection of runes (`runes/<domain>/` under a `deck.yaml` root) |
| **cast** | a named selection of runes across domains, for a use case (e.g. `development`) |
| **target** | the project you're currently working on — the repo rune deploys *into* |
| **`.rune`** | a consumer project's manifest: which deck, which runes/casts |
| **`.manifest`** | rune's record (per provider dir) of what it deployed, for drift/clean/doctor |

Two roles: you **author** a deck, and you **consume** it in a target.

## Setup (once)

```sh
export PATH="$HOME/.cargo/bin:$PATH"
rune --version              # 0.4.0 — commit hash now updates on every build
rune setup                  # guided: discovers decks under ~/Developer, persists the choice
rune config                 # table of resolved keys; get/set/unset/path for scripting
rune completion install          # writes to the shell standard location (bash|zsh|fish|nushell)
rune skill install          # teach AI agents on this machine to drive rune
```

**Review:** `setup` finds the real deck and never overwrites an existing choice;
`config get deck` prints the raw path (scriptable, exit 1 when unset); `config path`
names `~/.config/rune/config.yaml`; completions actually complete `rune sp<TAB>`.

---

## Part 1 — the flagship consumer flow

```sh
rune init N4M3Z/tour-1 --brief "Tour run"      # scaffold; --lang rust|shell|python
rune target N4M3Z/tour-1                          # bind it; rune target --list / rune target -
rune add --cast development                       # stage a cast
rune skill add deslop                             # stage by kind + bare name
rune agent add TheOpponent                        # same for agents, rules, hooks
rune rule add Deslop
rune add development/skills/deslop                # fully qualified ids still work
rune context                                      # where am I, what is staged, what next
cd ~/Agents/tour-1
rune tui --edit                                   # checkbox editor, I installs
rune install                                      # deploy to all providers
rune drift --target .                             # clean = deployed matches source
rune doctor --target .                            # ok / modified / missing / orphan
```

**Review:**
- `rune add` from a directory without `.rune` asks before staging into the bound
  target; EOF and non-interactive runs refuse — nothing lands elsewhere without consent.
- `rune skill add <name>` resolves the bare name to `<domain>/skills/<name>`; a name that
  exists in two domains errors listing both, and `<domain>/<name>` disambiguates.
- `rune context` names the acting root and role (consumer/deck/module/plain), the
  selection, per-provider deploy state, and a sensible `next:` step. `--json` for agents.
- Skills deploy inside the `rune` skills-directory plugin (`.claude/skills/rune/`) and
  load namespaced: `/rune:deslop`, `/rune:version-control`.
- The full `all` cast deploys 145 files under `.claude` (the rune plugin tree plus loose
  rules) and 141 into each of `.codex`, `.gemini`, `.opencode`.

---

## Part 2 — the TUI (`rune tui`)

Launch from a target or `rune tui --source <deck>`. `?` opens the keymap.

- **Miller columns** decks → kinds → runes; `/` filters in-pane; `!` problems only.
- **Code view**: `j/k`, `12j` counts (Esc cancels), `gg/G/zz`, `[[`/`]]` section jumps,
  `/` search with `n/N`, `V` visual select.
- **Comments**: `c` opens the tuicr-style box; Enter saves, Esc (twice when dirty)
  cancels; Tab cycles the kind. `rune review list|export` shows what persisted.
- **Edit**: `e` in-TUI editor, `E` `$EDITOR`, `o` creates a `user/` override.
- **Provenance tab**: the full SLSA statement per deployed rune.

**Review:** wheel scroll moves the viewport only, never the selection or a visual mode;
cursor survives fullscreen and tab switches; comment ranges export with source lines.

---

## Part 3 — deck authoring and integrity

```sh
rune status --source <deck>       # one-shot dashboard: decks, runes, casts, changes, validate
rune validate --source <deck>     # schemas + structure, fast (~0.2s)
rune validate --scan              # + gitleaks/semgrep — commit and push hooks only
rune adopt <dir> --module <target> --name <kebab-name> --source-url <url>
                                  # tree adoption: SKILL.md aligned, companions byte-for-byte,
                                  # per-file provenance sidecars; --dry-run first
rune provenance --target <project>/.claude
rune release <domain> --source <deck>
rune dashboard --source <deck>    # read-only web dashboard
```

**Review:** `validate` rejects a PascalCase skill `name` (kebab-case is enforced at author
time); adoption of a tree with Python/HTML companions copies them verbatim and each file
gets a sidecar; `--dry-run` prints the plan without writing.

---

## Part 4 — the spec lifecycle

```sh
rune spec propose add-widget --capability widgets
rune spec list                    # active changes; rune spec ls works too
rune spec list --specs            # canonical capability specs with requirement counts
rune spec show add-widget         # one change: state · tasks · proposal · deltas
rune spec context add-widget      # agent-ready work order
rune spec doctor                  # relationship health; exit 1 on broken changes
rune spec archive add-widget      # refuses unchecked tasks; --abandon to drop
```

Templates and mdschemas are overridable: drop replacements under `templates/spec/` or
`schemas/` at the source root and `spec propose`/`validate` prefer them over the embedded
copies — copying updated upstream templates in is a plain file replace.

**Review:** `show` disambiguates a name that is both a change and a spec by listing both
forms; `doctor` flags a change without proposal or delta as an error, an empty checklist
and a complete-but-unarchived change as warnings, and reports `spec tree healthy: …`
otherwise; try each in a throwaway copy of the deck.

---

## Part 5 — plumbing

```sh
rune assemble --source <deck>          # transform into build/ without deploying
rune deploy | rune copy                # deploy prebuilt / raw copy
rune exec <skill> -- <args>            # run a script bundled in a skill
rune launch <tool> --dry-run           # coding tool through middleware
rune find <query> · rune watch <cmd>   # relevance search over watched locations
```

---

## Suggested order

1. Setup block, then Part 1 end-to-end with a fresh slug — the 80% path.
2. Part 2 TUI, pressing every key.
3. Part 3 against the real deck (read-only commands are safe; adopt into a scratch module).
4. Part 4 in a temp copy of the deck.
5. Report anything surprising.
