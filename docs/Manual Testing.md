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
rune --help           # groups: Flow (setup, init, target, add, context, tui, dashboard, install, review), Spec, Deck, Plumbing (incl. skill, completion)
```

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
# [ISSUE] completion doesn't work even after installing and restarting the shell
rune completion install               # auto-detects $SHELL, writes the standard location (zsh, bash, fish, nushell)
# [NOTE] showing frontmatter would deserve its own command, the tui has a way to do it so just use that method
rune skill show | head -5      # frontmatter: name rune, current version
# [ISSUE] it should be installing into tempdir/.claude, no??
rune skill install --dir "$(mktemp -d)"   # prints installed → …/rune/SKILL.md; rerun prints unchanged
```

Expected: `setup --json` emits pure JSON (no prompt text); `config unset` accepts every key `config` lists; `skill install` refuses a symlinked destination.

## 3. Scaffold and bind a target

```sh
export RUNE_TARGETS="$(mktemp -d)"
# [NOTE] it should be possible to review the repo template, perhaps with the tui and also with some listing command, also CLAUDE.md should just include @AGENTS.md and nothing else
rune init demo --lang shell --purpose tool --brief "Manual init target"
cd "$RUNE_TARGETS/demo"
# [NOTE] the test command seems to do nothing??
test -x bin/demo && test -x .githooks/pre-commit
git config --get core.hooksPath       # .githooks
rune target .                          # binds this working repo as the target
# [NOTE] it shouldn't be necessary to indicate source deck if we already set the deck in config
rune add --source "$DECK" --cast development
# [NOTE] what the hell is an agentskills provider?? 
rune context                          # root (consumer) · selection · providers · next: rune install
# [NOTE] in the edit view, it's switch the oder of targer cdot cast to cast cdot target, and I think pressing tab should show me the artifact view like in the full tui
rune tui --edit                       # checkbox editor with the cast selected
# [NOTE] we didnt get to choose a provider, is there a rune provder command? or maybe the tui --edit should also show a selector
rune install
```
<!-- [NOTE] check if .gemini is still valid or if they changed it to .agy or something -->


Expected: init lists `base`, `lang/shell`, `purpose/tool` and makes one commit; `rune context` shows the staged cast and flips its `next:` suggestion to `rune doctor` once all providers are deployed.

## 4. The target-redirect note

```sh
cd "$(mktemp -d)" && rune add --cast development
```

Expected: an interactive prompt `no .rune here; stage into the bound target at …? [Y/n]`; answering n (or EOF, or any non-interactive run) cancels with `staging cancelled` and writes nothing. Nothing ever lands outside the current directory without an explicit yes.

## 5. Validate the deck

<!-- [NOTE] Seems we are installing an invalid template: -->
<!--  rune validate --scan

 validation
   ✗ module.yaml — missing required file: module.yaml
   ✗ defaults.yaml — missing required file: defaults.yaml
   ✓ README.md
   ✓ LICENSE
   ✓ INSTALL.md
   ✓ .gitattributes
   ⚡ .manifest — .manifest: missing — run rune install to establish baseline
   ✓ trailing whitespace

 ✓ 8 checked  ⚡ 1 warning  ✗ 2 errors -->

```sh
cd "$DECK" && rune validate          # fast (~0.2s), aggregate over all domains, no errors
rune validate --scan                 # + gitleaks and semgrep, the commit/push-hook mode
cd "$(mktemp -d)" && rune validate   # refuses: not a rune source; --force overrides
```

Expected: a PascalCase skill `name` in any `SKILL.md` fails plain `validate` with a pattern error; all shipped skills are kebab-case; validate never walks a directory without `deck.yaml`/`module.yaml` unless forced.

Lint warnings (non-blocking): validate also warns when a skill description lacks trigger phrasing ("use when", "invoke", …), when the name contains `claude`/`anthropic` or diverges from its directory, when name/description exceed the 64/1024 agentskills.io limits, when angle brackets are unmatched, or when the body is under 50 chars. To see one fire, temporarily blank a shipped skill's description trigger and rerun; the run stays exit 0 with a `warning:` line.

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
# [NOTE] drift should show primarly the drifted results, only with a param it should show all consistent files
rune drift --target .                 # flags Deslop.md as modified
# [NOTE] rune doctor print is ugly, improve it, just look at crex tui, run it and compare some views and use them to inspire
rune doctor --target .                # modified 1 · left untouched
```

## 7. Four providers, all cast

```sh
T=$(mktemp -d) && cd "$T" && rune target .
rune add --source "$DECK" --cast all >/dev/null && rune install >/dev/null
# [NOTE] I dont understand the purpose of for loop we are using here
for p in .claude .codex .gemini .opencode; do echo "$p: $(find $p -type f | wc -l)"; done
```

<!-- [NOTE] assembly rules should be configurable too, the kebab-case assembly rule for example, there used to be this assembly ruleset per provider that was there to achieve the necessary transformations -->
Expected: 145 files under .claude (the plugin manifest, merged hooks.json, and a second .manifest for the plugin root join the tree), 141 in each other provider directory. Claude Code loads the tree as the rune@skills-dir plugin, so skills invoke as /rune:<name>.

## 8. Qualified ids and kind-scoped add

```sh
T=$(mktemp -d) && cd "$T" && rune target .
rune skill add version-control --source "$DECK"   # bare name → development/skills/version-control
rune agent add TheOpponent                        # → council/agents/TheOpponent
rune rule add Deslop                              # → development/rules/Deslop
rune hook add safety-net                          # → development/hooks/safety-net
# [NOTE] I want all print outs of rune to use color, every response of rune
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

<!-- [NOTE] spec deserves its own document -->
## 10. Spec lifecycle

```sh
D=$(mktemp -d) && cp -R "$DECK"/* "$D/" && cd "$D"
rune spec propose add-widget --capability widgets
# [NOTE] ugly printout
rune spec ls                          # draft  add-widget  0/3 (alias for list)
rune spec ls --sort progress          # least-complete changes first, name as tiebreaker
# [NOTE] without completion this is entirely useless
rune spec show add-widget             # add-widget · draft · 0/3 tasks, then the work order
rune spec doctor                      # warning: no checked tasks yet, exit 0
#   check the [x] boxes in docs/changes/add-widget/tasks.md, then:
rune spec doctor                      # warning: complete; archive with rune spec archive add-widget
rune spec archive add-widget
rune spec list --specs                # widgets · N requirement(s)
rune spec show widgets                # prints the canonical capability spec
```
<!-- [NOTE] fundamentally, we should have the same way to handle the rune repos, so that deserves a subcommand too and it should have similar controls around mintlify. Have a look at mintlify cli, perhaps we can steal some of it -->

Template source of truth: drop a replacement `templates/spec/proposal.md` (or `tasks.md`, `delta-spec.md`, `design.md`, `schemas/*.mdschema`) at the source root and `spec propose`/`rune validate` prefer it over the embedded copy — updating from upstream (e.g. OpenSpec) is a plain file copy.

Artifact parity extras: `rune spec propose big-change --capability alpha --capability beta --design` scaffolds one delta per capability, a `## Capabilities` section in the proposal, and `design.md` (which `spec context`/`show` include). `rune spec archive <id> --abandon -y` works scripted. Root divergence from OpenSpec is intentional: rune roots at `docs/changes/`, OpenSpec hardcodes `openspec/`; parity is at the artifact and dialect level, not the root path.

Expected: `doctor` exits 1 only when a change is structurally broken (no proposal, no delta); `show` on a name that is both a change and a spec errors listing both forms.

## 11. TUI

```sh
cd "$DECK" && rune tui
```
<!-- [NOTE] I have many thoughts on the tui, let's prepare a separate guide and I will go through it, similar to rune spec -->


Miller-column navigation, `/` in-pane filter, `!` problems-only, History batched loading. Code tab: `12j`, `5G`, `gg`, `zz`, `]]`/`[[`, `/` + `n/N`, `V` + `c` range comments; Enter saves, Esc (twice when dirty) cancels; wheel scroll moves only the viewport. Then:

```sh
rune review list --source "$DECK"
rune review export --source "$DECK" --format markdown
```

<!-- [NOTE] rune review list --source "$DECK"
error: unexpected argument '--source' found

Usage: rune review list [OPTIONS]

For more information, try '--help'.

some bug I guess-->

## 12. Adopt a skill tree

<!-- [NOTE] I didnt run this yet, does run adopt run through the adoption process? if so, how? it just imports the artifacts? that's the wrong approach I think, we should have two commands if you want one to just copy shit from a module and another one to run a harness and start the adoption process which we still need to properly formalize -->

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
