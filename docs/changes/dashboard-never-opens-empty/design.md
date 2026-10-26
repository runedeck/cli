# TUI first run design

## Approach

One palette struct in `src/tui/styles.rs` derives the TUI's interface colors from
`theme::current()`. The five theme tones carry the meaning (accent for focus and keys, good and bad
for diffs and states, alert for the cursor, violet for headings). A `light` flag on the theme
picks the backgrounds and text tones, so a light palette flips panel and text without a second setting. This
beat adding background tokens to every theme because the tones already exist and the flag is one
bit per palette.

## Structure

- `ThemeTones` carries `light`. The two light palettes, `rune-light` and `catppuccin-latte`, set
  it.
- `styles::Palette::from_theme` maps tones to the TUI roles. One accessor function returns each
  role, so a call site names one function.
- `App` detects provider states once at load through the shared detection registry and reads the
  bound target once. The status bar renders both beside the deck name.
- `App::is_first_run` is true after a completed scan that found no deck and no modules. `render`
  then draws the first-run panel in place of the three columns and a matching footer hint. A
  failed scan sets `ScanState::Failed`, and the footer names its error.
- The help overlay title carries the version and the close keys. Group labels and keys use the
  theme.

## Risks

- Detection at load adds filesystem reads before the first frame. They are bounded evidence
  checks and run once.
- The TUI paints RGB values only. The ANSI fallback of each tone serves `Sheet` output, so a
  terminal without truecolor receives the RGB values as they are.
- Syntax highlighting in the Code tab and in rendered markdown keeps the fixed `base16-ocean.dark`
  colors whatever the theme. The wrap marker and the preview position counter keep named grays.
- Golden snapshots pin the default dark theme. A theme change in tests must install one first.
