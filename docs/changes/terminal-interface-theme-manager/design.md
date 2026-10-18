# Herdr Theme System Design

## Approach

Resolve the theme once at dispatch and hand every render path one palette. This beat per-command
theme reads because precedence stays in one place: `--no-color`, then `NO_COLOR`, then
non-terminal output, then the resolved theme. Palettes ship as data from their canonical upstream
projects instead of copies from herdr, so attribution stays first party.

## Structure

- A theme module holds the named palettes as data and the resolution function.
- `theme.name` selects the palette. `theme.auto_switch` with `theme.dark_name` and
  `theme.light_name` follows the host appearance when the terminal reports it.
- `theme.custom` overrides single tokens on the resolved base.
- `Sheet` keeps its API. Its tones read the resolved palette instead of constants.
- The TUI palette derives from the same resolved palette.
- The config reference gains the theme section through the existing Schemars path.

## Risks

- Appearance detection is unreliable in one-shot CLI runs. Detection stays best effort and the
  configured default wins when the terminal stays silent.
- Palette licensing: canonical upstreams (Catppuccin, Tokyo Night, Nord) carry permissive
  licenses. Attribution lives beside the data and in the provenance sidecars.
- Golden tests break on palette drift. Tests pin one named theme and the plain mode.
- An unknown theme name must warn and keep the default, never abort a command.
