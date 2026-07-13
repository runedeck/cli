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
rune --version        # rune 0.4.0
rune --help           # subcommands include add, install, validate, drift, review, tui, dashboard, adopt, find, launch, watch
```

## 2. Scaffold and bind a quest

```sh
export RUNE_QUESTS="$(mktemp -d)"
rune init demo --lang shell --purpose tool --brief "Manual init quest"
cd "$RUNE_QUESTS/demo"
test -x bin/demo && test -x .githooks/pre-commit
! grep -E '\${(NAME|TITLE|BRIEF|OWNER)}' bin/demo Makefile
git config --get core.hooksPath       # .githooks
rune quest .                          # binds this working repo as the quest
rune add --source "$DECK" --cast development
rune tui --edit                       # opens the checkbox editor with the cast selected
rune install
```

Expected: init lists `base`, `lang/shell`, and `purpose/tool`, creates the quest
under `RUNE_QUESTS`, substitutes `demo` in the shell command and Makefile,
marks hooks and `bin/demo` executable, initializes `main`, and activates
`.githooks`. Quest binding records this repo; the editor shows the development
cast selected and installs the checked runes into the four provider targets.

## 3. Validate the deck

```sh
cd "$DECK" && rune validate
```

Expected: an aggregate report over the four decks (council, development, meta, research); ADR schema checks pass; no errors.

## 4. Fresh consumer, development cast

```sh
T=$(mktemp -d) && cd "$T"
rune config set deck "$DECK"
rune add --cast development
cat .rune             # version, deck source, cast: development
rune install          # deploys with a count, no warnings about skipped files
```

Inspect the target:

```sh
ls .claude/rules      # eight rules incl. Deslop.md, StageForReview.md
ls .claude/skills     # Brainstorming, DeliveryPipeline, Deslop, LearnFrom, SystematicDebug, VerifyCompletion, VersionControl
cat .claude/hooks/development/hooks.json    # command path rewritten to the deployed location
test -x .claude/hooks/development/safety-net.sh && echo executable
```

Drift must be clean, then detect a real edit:

```sh
rune drift --target .claude                 # clean, exit 0
echo tamper >> .claude/rules/Deslop.md
rune drift --target .claude                 # flags Deslop.md as modified
```

## 5. Four providers, all cast

```sh
T=$(mktemp -d) && cd "$T"
rune add --source "$DECK" --cast all >/dev/null && rune install >/dev/null
for p in .claude .codex .gemini .opencode; do echo "$p: $(find $p -type f | wc -l)"; done
```

Expected: 99 files in each provider directory.

## 6. Qualified ids and the ambiguity guard

```sh
T=$(mktemp -d) && cd "$T"
rune add --source "$DECK" development/skills/VersionControl   # single rune, ok
rune add development/Deslop
```

Expected: the second command fails loudly — `development/Deslop` is ambiguous (a rule and a skill share the name) and the error lists both candidates. Retry with `development/rules/Deslop`.

## 7. Pinned git install (the remote-consumer path)

```sh
T=$(mktemp -d) && cd "$T"
SHA=$(git -C "$DECK" rev-parse HEAD)
printf 'version: 1\nsources:\n  deck:\n    git: file://%s\n    ref: %s\nartifacts:\n  deck:\n    cast: development\n' "$DECK" "$SHA" > .rune
RUNE_GIT_ALLOW_FILE_URLS=1 rune install
```

Expected: same deployment as step 3, materialized from the pinned commit. Over HTTPS only the transport differs.

## 8. Legacy compatibility

```sh
T=$(mktemp -d) && cd "$T"
rune add --source "$DECK" --cast base >/dev/null
mv .rune .forge && rune install      # legacy manifest still resolves
mkdir .rune && rune install          # a directory named .rune does not shadow .forge
```

`FORGE_GIT_CACHE_DIR` is honored when `RUNE_GIT_CACHE_DIR` is unset; old provenance sidecars with forge URIs still verify.

## 9. TUI

```sh
cd "$DECK" && rune tui
```

Expected: header shows 4 modules; sections include Decks, Casts, History. Try: Miller-column navigation decks → kinds → runes; `/` filters in-panel; `!` shows problems only; the casts section resolves membership; History renders the commit list batched (scroll keeps loading); wheel scroll moves the viewport without dragging the selection. Non-interactive render: `rune tui --snapshot`.

In an artifact's Code tab, verify the review controls: `12j`, `5G`, `gg`, and
`zz` move or position the line cursor; `]]`/`[[` jump Markdown sections; `/`
highlights matches incrementally and `n`/`N` repeat the search. Press `V`,
extend with `j`/`k` or a count, then `c` to comment the selected range. The
comment box starts in insert mode; `Esc` enters normal mode, where
`i`/`a`/`A`/`o`, `dd`, `x`, and `w`/`b` work. Save with `:w` or Ctrl-S. A
dirty `:q`, `q`, or normal-mode `Esc` asks for confirmation before discarding.
Mouse-wheel scrolling must move only the viewport while a visual selection is
active. `;e` opens the cast editor and `;q` quits.

After saving comments, verify the persisted and agent-facing forms:

```sh
rune review list --source "$DECK"
rune review export --source "$DECK" --format markdown
```

Expected: `.rune-comments.yaml` contains `end_line` only for ranges; legacy
single-line records still load. The export groups comments by file, includes
the selected source lines, and matches what `y` copies from the TUI.

## 10. Dashboard

```sh
cd "$DECK" && rune dashboard
```

Expected: a loopback URL; panels for decks (counts, validation), casts (resolved sizes), and target deploy status; entirely read-only; deck routes 404 outside a deck.

## 11. Repo hooks

```sh
cd ~/Developer/runedeck/rune
echo >> README.md && git add README.md && git commit -m "test: hook check"
```

Expected: prek runs whitespace/yaml/shellcheck/fmt/clippy/test/semgrep, gitleaks scans staged content, `rune validate` checks ADR schemas — all before the commit lands. Undo: `git reset --hard HEAD~1` (or ask the resident agent).

## 12. Guardrails worth seeing fail

```sh
cd "$DECK"
sed -i '' 's/^schema: 1/schema: 2/' deck.yaml
rune validate         # hard error naming found schema 2 vs supported 1
sed -i '' 's/^schema: 2/schema: 1/' deck.yaml

touch runes/development/rules/junk.txt
rune validate         # or any install: a named warning for the unsupported file, never silence
rm runes/development/rules/junk.txt
```

## Reference

Backups: `~/Data/Claude/backups/runedeck-*.tgz`. Checkpoint tags in the rune repo: `checkpoint-stage-a`, `checkpoint-complete`. ADRs: `docs/decisions/` in both repos.
