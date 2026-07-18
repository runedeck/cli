# TUI Walkthrough

Review session for `rune tui`. Each step names the input and the expected result. Run from the deck: `cd $DECK && rune tui`.

## Navigation (Browse tab)

| Input | Expected |
|-------|----------|
| `h`/`l`, arrows | Miller columns: domain → kind → artifact; focus slides without losing selection |
| `j`/`k`, `12j`, `5G`, `gg`, `G` | Cursor moves with counts; viewport follows the cursor |
| `/` | In-pane filter; typing narrows live; Esc clears |
| `!` | Problems-only toggle: only artifacts with validation findings remain |
| Enter | Detail pane: rendered frontmatter fields + body preview |
| `q` | Quit from anywhere without prompts when nothing is dirty |

## Edit mode (`rune tui --edit`)

| Input | Expected |
|-------|----------|
| open | Checkbox editor over the staged selection, header reads `cast · target` |
| space | Toggles the artifact under the cursor |
| Tab | Artifact preview pane, same renderer as the Browse detail view |
| provider column | Selector listing providers with enabled state; toggling persists to the selection |
| Enter / Esc | Save / cancel; dirty state requires Esc twice |

## Code tab (review)

`12j`, `5G`, `gg`, `zz`, `]]`/`[[` hunk jumps, `/` + `n`/`N` search, `V` + `c` range comments; Enter saves a comment, Esc (twice when dirty) cancels. Wheel scroll moves only the viewport, never the cursor (selection-moving input and viewport-scrolling input stay separate).

## History tab

Batched loading: scrolling near the end fetches the next batch; no full-history stall on large repos.

## Planned changes under review (Phase 5)

- Edit view header order becomes `cast · target`; Tab opens the artifact preview (both listed above as expected state).
- Provider selector joins the edit view; `rune provider` is the scriptable counterpart.
- Styling from the shared truecolor sheet (Phase 4) replaces per-view ad hoc colors.

## Known non-goals

- The TUI is not the deploy surface: `rune install` stays a CLI action; the TUI edits selections and reviews content.
