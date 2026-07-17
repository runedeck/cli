# Manual check — rune since v0.4.0

Everything below is landed and installed. The binary `rune --version` still prints the old
commit hash `646715f` — that is a known cosmetic `build.rs` stamp bug, not a stale binary.
Behaviour is current: `rune --help` shows `spec`, `status`, `doctor`, and the rest of the grouped commands.

Setup (once):

```sh
export PATH="$HOME/.cargo/bin:$PATH"
rune config set deck ~/Developer/runedeck/runedeck   # already set; harmless to repeat
```

Pick a fresh slug for the flagship walkthrough (avoids collisions):

```sh
rune init N4M3Z/check-1 --brief "Manual check run"
rune quest N4M3Z/check-1
rune add --cast development
```

---

## A. Regressions to confirm fixed (the bugs you hit last time)

1. **init makes a real commit** — `git -C ~/Agents/check-1 log --oneline` shows one
   `chore: scaffold from skeleton` commit (last time the repo was commitless).

2. **CLI output is legible** — `rune quest` / `rune add` above printed
   `bound quest 'check-1' → …` and `staged cast 'development' … / next: rune install`,
   not a bare `--source` dump.

3. **Open the TUI** — `cd ~/Agents/check-1 && rune tui`.
   - **Footer everywhere**: every section (Decks / list / detail / `--edit`) shows a hint bar
     at the bottom. Last time list/edit modes were blank.
   - **No warning bleed**: the rune list is clean — no `warning: cannot determine git freshness`
     text overwriting rows.

4. **`rune tui --edit`** — the checkbox editor shows a footer
   `Space toggle · j/k move · n/p deck · I install · q quit`. Tick a few, press `I`.

5. **Code view shows the real file** — drill into a rule or agent (not just a skill), open the
   Code tab. It renders the actual source bytes, never "source unavailable".

6. **Number keys don't trap you** — in the Code tab press `1` `2`. The footer shows
   `count: 12 — press j/k to repeat, Esc to cancel`. Press `Esc` → it clears. Press a letter
   that isn't a motion → count clears and the letter acts. No stuck "999999" mode.

7. **Comment with `c` then Enter** — in the Code tab, put the cursor on a line, press `c`,
   type text, press **Enter**. The comment saves and the box closes (last time Enter did
   nothing — it was a vim-modal needing `:w`). Esc cancels.

8. **Comment box looks like tuicr** — the inline comment box has the `│` border prefix and a
   kind badge, ported from tuicr's renderer.

9. **Cursor survives fullscreen / tab 2** — set the cursor on a line, toggle fullscreen, switch
   to the Diff tab and back. The cursor stays on the same logical line (last time it got lost).

10. **Provenance shows the full SLSA payload** — open the Provenance (`v`) tab on a deployed
    rune. It renders predicate type, builder id, every subject/material sha256 digest,
    invocation, and metadata — the complete in-toto statement, scrollable.

---

## B. New commands to explore

11. **`rune status`** — from the deck or a consumer, a one-shot dashboard:

    ```sh
    rune status --source ~/Developer/runedeck/runedeck
    ```

    Summary line (decks · runes by kind · casts · change states · validate counts), then
    Changes (progress bars), Specifications, and Deploy targets. `rune status --json` for the
    machine form.

12. **`rune doctor`** — deployment integrity, never touches your edits:

    ```sh
    cd ~/Agents/check-1 && rune install >/dev/null && rune doctor --target .
    #   → ok N · modified 0 · missing 0 · orphan 0
    echo tamper >> .claude/rules/Deslop.md && rune doctor --target .
    #   → modified 1, and: "left untouched; use `rune install --force` to replace it"
    rune doctor --target . --repair        # restores missing, quarantines orphans; leaves your edit
    ```

13. **Spec-driven lifecycle** (the openspec adoption — lives under `docs/`, no `openspec/` folder).
    Try it in a throwaway copy of the deck so you don't touch the real one:

    ```sh
    D=$(mktemp -d) && cp -R ~/Developer/runedeck/runedeck/* "$D/" && cd "$D"
    rune spec propose add-widget --capability widgets # scaffolds docs/changes/add-widget/
    rune spec list                                    # → draft  add-widget  0/3
    rune spec context add-widget                      # Markdown work order; add --json for agents
    #   edit docs/changes/add-widget/tasks.md, check the boxes ([x]), then:
    rune spec archive add-widget                      # refuses if any task unchecked (lists them)
    rune spec archive add-widget -y                   # merges the delta into docs/specs/widgets/
    #   → "Archived … as merged", canonical spec created, change moved to docs/changes/archive/DATE-id/
    rune spec archive some-other-change --abandon     # the escape hatch: archives without merging
    ```

    The guardrails you approved: `proposal.md` links the ADR for the *why* (doesn't restate it),
    every change ends explicitly merged **or** `--abandon`ed (no silent rot), and the README
    states the complexity threshold (change folders are for multi-session work; small fixes skip
    the ceremony).

---

## Known follow-ups (not blocking your check)

- `rune --version` prints a stale commit hash (cosmetic `build.rs` rerun gap).
- The pre-commit hook hard-depends on `rune` being on PATH — breaks a fresh clone / CI.

## Still queued (your "Both, sequenced" pick — not yet built)

- crex visual idioms folded into the pane TUI (adaptive palette, grouped emoji help, digit-jump).
- `rune shell` — a crex-style REPL surface mirroring the CLI 1:1.

Say the word and I hand these to Sol; otherwise they wait until after your manual check.
