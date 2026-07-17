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
rune --version        # rune 0.4.0 (<current commit>) — the hash tracks HEAD now
rune --help           # groups: Flow (setup, init, target, add, context, tui, dashboard, install, review), Spec, Deck, Plumbing (incl. skill, completion)
```

## 2. First-run surface

```sh
rune setup --defaults          # reports deck + target state without prompting
rune config path               # ~/.config/rune/config.yaml
rune config get deck           # raw value, exit 0; exit 1 when unset
rune config set owner tester && rune config get owner && rune config unset owner
rune completion print zsh | head -3   # a real #compdef script
rune completion install               # auto-detects $SHELL, writes the standard location
rune skill show | head -5      # frontmatter: name rune, current version
rune skill install --dir "$(mktemp -d)"   # prints installed → …/rune/SKILL.md; rerun prints unchanged
```

Expected: `setup --json` emits pure JSON (no prompt text); `config unset` accepts every key `config` lists; `skill install` refuses a symlinked destination.

## 3. Scaffold and bind a target

```sh
export RUNE_TARGETS="$(mktemp -d)"
rune init demo --lang shell --purpose tool --brief "Manual init target"
cd "$RUNE_TARGETS/demo"
test -x bin/demo && test -x .githooks/pre-commit
git config --get core.hooksPath       # .githooks
rune target .                          # binds this working repo as the target
rune add --source "$DECK" --cast development
rune context                          # root (consumer) · selection · providers · next: rune install
rune tui --edit                       # checkbox editor with the cast selected
rune install
```

Expected: init lists `base`, `lang/shell`, `purpose/tool` and makes one commit; `rune context` shows the staged cast and flips its `next:` suggestion to `rune doctor` once all providers are deployed.

## 4. The target-redirect note

```sh
cd "$(mktemp -d)" && rune add --cast development
```

Expected: a loud `note: no .rune here; acting on the bound target at …` line, then `already staged … → <target>/.rune`. The write never lands in the current directory silently.

## 5. Validate the deck

```sh
cd "$DECK" && rune validate          # fast (~0.2s), aggregate over all domains, no errors
rune validate --scan                 # + gitleaks and semgrep, the commit/push-hook mode
cd "$(mktemp -d)" && rune validate   # refuses: not a rune source; --force overrides
```

Expected: a PascalCase skill `name` in any `SKILL.md` fails plain `validate` with a pattern error; all shipped skills are kebab-case; validate never walks a directory without `deck.yaml`/`module.yaml` unless forced.

## 6. Fresh consumer, development cast

```sh
T=$(mktemp -d) && cd "$T"
rune target .                          # rebind so this directory is the acting root
rune add --cast development
rune install
ls .claude/rules                      # Deslop.md, StageForReview.md, … (rules stay PascalCase)
ls .claude/skills                     # brainstorming, delivery-pipeline, deslop, learn-from, systematic-debug, verify-completion, version-control
test -x .claude/hooks/development/safety-net.sh && echo executable
rune drift --target .                 # clean, exit 0
echo tamper >> .claude/rules/Deslop.md
rune drift --target .                 # flags Deslop.md as modified
rune doctor --target .                # modified 1 · left untouched
```

## 7. Four providers, all cast

```sh
T=$(mktemp -d) && cd "$T" && rune target .
rune add --source "$DECK" --cast all >/dev/null && rune install >/dev/null
for p in .claude .codex .gemini .opencode; do echo "$p: $(find $p -type f | wc -l)"; done
```

Expected: 141 files in each provider directory.

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
printf 'version: 1\nsources:\n  deck:\n    git: file://%s\n    ref: %s\nartifacts:\n  deck:\n    cast: development\n' "$DECK" "$SHA" > .rune
RUNE_GIT_ALLOW_FILE_URLS=1 rune install
```

Expected: same deployment as step 6, materialized from the pinned commit.

## 10. Spec lifecycle

```sh
D=$(mktemp -d) && cp -R "$DECK"/* "$D/" && cd "$D"
rune spec propose add-widget --capability widgets
rune spec ls                          # draft  add-widget  0/3 (alias for list)
rune spec show add-widget             # add-widget · draft · 0/3 tasks, then the work order
rune spec doctor                      # warning: no checked tasks yet, exit 0
#   check the [x] boxes in docs/changes/add-widget/tasks.md, then:
rune spec doctor                      # warning: complete; archive with rune spec archive add-widget
rune spec archive add-widget
rune spec list --specs                # widgets · N requirement(s)
rune spec show widgets                # prints the canonical capability spec
```

Template source of truth: drop a replacement `templates/spec/proposal.md` (or `tasks.md`, `delta-spec.md`, `schemas/*.mdschema`) at the source root and `spec propose`/`rune validate` prefer it over the embedded copy — updating from upstream (e.g. OpenSpec) is a plain file copy.

Expected: `doctor` exits 1 only when a change is structurally broken (no proposal, no delta); `show` on a name that is both a change and a spec errors listing both forms.

## 11. TUI

```sh
cd "$DECK" && rune tui
```

Miller-column navigation, `/` in-pane filter, `!` problems-only, History batched loading. Code tab: `12j`, `5G`, `gg`, `zz`, `]]`/`[[`, `/` + `n/N`, `V` + `c` range comments; Enter saves, Esc (twice when dirty) cancels; wheel scroll moves only the viewport. Then:

```sh
rune review list --source "$DECK"
rune review export --source "$DECK" --format markdown
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
