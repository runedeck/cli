# Manual Testing

A hands-on walkthrough of the rune + deck system. Every step names its expected result. `DECK` is the content repo; steps assume `~/.cargo/bin` on PATH.

```sh
export DECK=~/Developer/runedeck/runedeck
export PATH="$HOME/.cargo/bin:$PATH"
```

## 1. Build and install

```sh
cd ~/Developer/runedeck/rune
cargo install --path .
rune --version        # rune 0.5.0 (<current commit>) — the hash tracks HEAD
rune --help           # runic wordmark, then groups: Flow, Spec, Deck, Plumbing
```

Expected: on a TTY the wordmark renders in color (cyan sigil, dim tagline); piped output is plain text.

## 2. First-run surface

```sh
rune setup --defaults          # reports deck + target state without prompting
rune config path               # ~/.config/rune/config.yaml
rune config get deck           # raw value, exit 0; exit 1 when unset
rune config set owner tester && rune config get owner && rune config unset owner
#   owner = default owner segment for init slugs: `rune init demo` scaffolds
#   under <targets>/<owner>/demo; an explicit `rune init acme/demo` overrides it
rune completion print zsh | head -3   # a real #compdef script
rune completion print nushell | head -3
rune completion install               # writes the standard location AND clears ~/.zcompdump*
rune skill show | head -5      # frontmatter: name rune, current version
rune skill install --dir "$(mktemp -d)"   # installs under <dir>/.claude/skills/rune/SKILL.md
```

Expected: `completion install` reports the cleared completion cache and completions work after a shell restart; `setup --json` emits pure JSON; `config unset` accepts every key `config` lists; `skill install` refuses a symlinked destination and defaults to `~/.claude/skills` when `--dir` is omitted.

## 3. Scaffold and bind a target

```sh
export RUNE_TARGETS="$(mktemp -d)"
rune init demo --lang shell --purpose tool --dry-run   # plan only, writes nothing
rune init demo --lang shell --purpose tool --brief "Manual init target"
cd "$RUNE_TARGETS/demo"
test -x bin/demo && test -x .githooks/pre-commit && echo hooks-ok
git config --get core.hooksPath       # .githooks
ls -d private public assets .jj       # the workshop layout + jj colocation
git rev-parse --verify HEAD           # fails: workshop init never auto-commits
rune target .                          # binds this working repo as the target
rune add --cast development            # the configured deck supplies the source
rune context                          # root (consumer) · selection · providers · next: rune install
rune tui --edit                       # checkbox editor with the cast selected
rune install
rune validate                          # consumer checks: .rune parses, per-provider manifests
```

Expected: init lists `base`, `lang/shell`, `purpose/tool`. Under the targets root init runs in workshop mode: the private/public/assets layout lands, jj colocates when installed, and the first commit stays yours (`--workshop` forces the mode elsewhere; `--spine` gives a plain project the jj colocation; outside the targets root a plain init still commits the scaffold). The composed `.gitignore` carries both the base entries and the lang fragment; `rune validate` in the scaffolded project runs consumer checks (no module.yaml errors) so the pre-commit hook passes.

## 4. The target-redirect note

```sh
cd "$(mktemp -d)" && rune add --cast development
```

Expected: an interactive prompt `no .rune here; stage into the bound target at …? [Y/n]`; answering n (or EOF, or any non-interactive run) cancels with `staging cancelled` and writes nothing. Nothing ever lands outside the current directory without an explicit yes.

## 5. Validate the deck

```sh
cd "$DECK" && rune validate          # fast (~0.2s), aggregate over all domains, no errors
rune validate --scan                 # + gitleaks and semgrep, the commit/push-hook mode
cd "$(mktemp -d)" && rune validate   # refuses: not a rune source; --force overrides
```

Expected: a PascalCase skill `name` in any `SKILL.md` fails plain `validate` with a pattern error; all shipped skills are kebab-case; validate never walks a directory without `deck.yaml`/`module.yaml`/`.rune` unless forced. A root carrying both `module.yaml` and `.rune` gets both check sets.

Lint warnings (non-blocking): validate also warns when a skill description lacks trigger phrasing ("use when", "invoke", …), when the name contains `claude`/`anthropic` or diverges from its directory, when name/description exceed the 64/1024 agentskills.io limits, when angle brackets are unmatched, or when the body is under 50 chars.

## 6. Fresh consumer, development cast

```sh
T=$(mktemp -d) && cd "$T"
rune target .                          # rebind so this directory is the acting root
rune add --cast development
rune install
ls .claude/rules                      # Deslop.md, StageForReview.md, … (rules stay PascalCase)
ls .claude/skills/rune/skills         # brainstorming, delivery-pipeline, deslop, … (the rune plugin root)
test -x .claude/skills/rune/hooks/development/safety-net.sh && echo executable
cat .claude/skills/rune/.claude-plugin/plugin.json   # the namespace source: name rune
rune drift --target .                 # clean, exit 0
echo tamper >> .claude/rules/Deslop.md
rune drift --target .                 # flags Deslop.md as modified
rune doctor --target .                # modified 1 · left untouched
```

## 7. Four providers, all cast

```sh
T=$(mktemp -d) && cd "$T" && rune target .
rune add --source "$DECK" --cast all >/dev/null && rune install >/dev/null
#   count deployed files per provider directory to confirm all four landed:
for p in .claude .codex .gemini .opencode; do echo "$p: $(find $p -type f | wc -l)"; done
```

Expected: 145 files under .claude (the plugin manifest, merged hooks.json, and a second .manifest for the plugin root join the tree), 141 in each other provider directory. Claude Code loads the tree as the rune@skills-dir plugin, so skills invoke as /rune:<name>.

## 7b. Providers and assembly rules

```sh
rune provider                        # name · enabled · target · plugin per provider
rune provider disable gemini         # writes providers.gemini.enabled into ./config.yaml
rune provider enable gemini
```

Expected: agentskills ships disabled (deploys only via `--provider agentskills`); assembly transforms are named rules per provider in `defaults.yaml` (`kebab-case`, `kebab-case-agents`, `remap-tools`, `strip-links`, `agents-to-toml`) and a module's `config.yaml` can override the list per provider.

## 8. Qualified ids and kind-scoped add

```sh
T=$(mktemp -d) && cd "$T" && rune target .
rune skill add version-control --source "$DECK"   # bare name → development/skills/version-control
rune agent add TheOpponent                        # → council/agents/TheOpponent
rune rule add Deslop                              # → development/rules/Deslop
rune hook add safety-net                          # → development/hooks/safety-net
rune skill add nonexistent                        # fatal: no skills rune named 'nonexistent'
rune add development/skills/version-control       # fully qualified ids still work
```

Expected: each kind command echoes the fully qualified id it staged; a bare name present in two domains errors listing both and `<domain>/<name>` disambiguates.

## 9. Pinned git install (the remote-consumer path)

```sh
T=$(mktemp -d) && cd "$T" && rune target .
SHA=$(git -C "$DECK" rev-parse HEAD)
printf 'version: 1\nsources:\n  deck:\n    git: file://%s\n    ref: %s\nrunes:\n  deck:\n    casts: development\n' "$DECK" "$SHA" > .rune
RUNE_GIT_ALLOW_FILE_URLS=1 rune install
```

Expected: same deployment as step 6, materialized from the pinned commit.

## 10. Spec lifecycle

Covered in depth by the dedicated Spec walkthrough (docs/walkthroughs/). Quick pass:

```sh
D=$(mktemp -d) && cp -R "$DECK"/* "$D/" && cd "$D"
rune spec propose add-widget --capability widgets
rune spec ls                          # draft  add-widget  0/3 (alias for list)
rune spec ls --sort progress          # least-complete changes first, name as tiebreaker
rune spec show add-widget             # add-widget · draft · 0/3 tasks, then the work order
rune spec doctor                      # warning: no checked tasks yet, exit 0
#   check the [x] boxes in docs/changes/add-widget/tasks.md, then:
rune spec doctor                      # warning: complete; archive with rune spec archive add-widget
rune spec archive add-widget
rune spec list --specs                # widgets · N requirement(s)
rune spec show widgets                # prints the canonical capability spec
```

Template source of truth: drop a replacement `templates/spec/proposal.md` (or `tasks.md`, `delta-spec.md`, `design.md`, `schemas/*.mdschema`) at the source root and `spec propose`/`rune validate` prefer it over the embedded copy — updating from upstream (e.g. OpenSpec) is a plain file copy.

Artifact parity extras: `rune spec propose big-change --capability alpha --capability beta --design` scaffolds one delta per capability, a `## Capabilities` section in the proposal, and `design.md` (which `spec context`/`show` include). `rune spec archive <id> --abandon -y` works scripted. Root divergence from OpenSpec is intentional: rune roots at `docs/changes/`, OpenSpec hardcodes `openspec/`; parity is at the artifact and dialect level, not the root path.

Expected: `doctor` exits 1 only when a change is structurally broken (no proposal, no delta); `show` on a name that is both a change and a spec errors listing both forms.

## 11. TUI

Covered in depth by the dedicated TUI walkthrough (docs/walkthroughs/). Quick pass:

```sh
cd "$DECK" && rune tui
```

Miller-column navigation, `/` in-pane filter, `!` problems-only, History batched loading. Code tab: `12j`, `5G`, `gg`, `zz`, `]]`/`[[`, `/` + `n/N`, `V` + `c` range comments; Enter saves, Esc (twice when dirty) cancels; wheel scroll moves only the viewport. Then:

```sh
rune review list --target "$DECK"
rune review export --target "$DECK" --format markdown
```

## 12. Adopt a skill tree

```sh
M=$(mktemp -d) && rune init --module "$M/scratch"
rune adopt <path-to-skill-dir> --module "$M/scratch" --name example-adopted --dry-run
rune adopt <path-to-skill-dir> --module "$M/scratch" --name example-adopted --source-url https://example.com/upstream
find "$M/scratch/skills/example-adopted" -name "*.yaml" -path "*.provenance*" | head
```

Expected: dry-run prints the plan without writing; the real run aligns `SKILL.md`, copies every companion byte-for-byte, and writes one provenance sidecar per file.

## 13. Guardrails worth seeing fail

```sh
cd "$DECK"
sed -i '' 's/^schema: 1/schema: 2/' deck.yaml
rune validate         # hard error naming found schema 2 vs supported 1
sed -i '' 's/^schema: 2/schema: 1/' deck.yaml
```

## Reference

Backups: `~/Data/Claude/backups/runedeck-*.tgz`. ADRs: `docs/decisions/` in both repos. Cleanup: `rune target -` restores your previous target binding after the temp-dir steps.
