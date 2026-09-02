# TUI First Run Design

## Approach

One palette struct in `src/tui/styles.rs` derives every TUI color from `theme::current()`. The
five theme tones carry the meaning (accent for focus and keys, good and bad for diffs and states,
alert for the cursor, violet for headings). Surfaces and text tones follow a `light` flag on the
theme, so a light palette flips panel and text without a second setting. This beat adding surface
tokens to every theme because the tones already exist and the flag is one bit per palette.

## Structure

- `ThemeTones` gains `light`. The two light palettes set it.
- `styles::Palette::from_theme` maps tones to the TUI roles; accessor functions replace the old
  constants, so call sites change one token each.
- `App` detects provider states once at load through the shared detection registry and reads the
  bound target once. The status bar renders both beside the deck name.
- `App::is_first_run` is true after a scan that found no deck and no modules. `render` then
  draws the first-run panel in place of the three columns and a matching footer hint.
- The help overlay title carries the version and the close keys; group labels and keys use the
  theme.

## Risks

- Detection at load adds filesystem reads before the first frame. They are bounded evidence
  checks and run once.
- A light palette on a terminal that ignores truecolor falls back to the ANSI tones; surfaces
  stay readable because text tones are basic colors.
- Golden snapshots pin the default dark theme; a theme change in tests must install one first.
