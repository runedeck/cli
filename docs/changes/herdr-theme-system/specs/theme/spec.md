## ADDED Requirements

### Requirement: Named theme selection

`theme.name` SHALL select one built-in palette, and an unknown name SHALL warn and keep the
default without aborting the command.

#### Scenario: Config names an unknown theme

- **WHEN** the user config sets `theme.name` to an unknown value
- **THEN** the command warns once, uses the default palette, and completes

### Requirement: Appearance pairing

With `theme.auto_switch` enabled, rune SHALL use `theme.light_name` on a light terminal and
`theme.dark_name` on a dark terminal when the terminal reports its appearance, and SHALL use the
configured default when it does not.

#### Scenario: Terminal reports a light background

- **WHEN** `theme.auto_switch` is true and the terminal reports a light background
- **THEN** the resolved palette is the one named by `theme.light_name`

### Requirement: Token overrides

`theme.custom` entries SHALL override single color tokens on the resolved base palette.

#### Scenario: Config overrides one token

- **WHEN** `theme.custom` sets one token to a valid color
- **THEN** that token uses the override and every other token keeps the base value

### Requirement: One palette source

`Sheet` output and the TUI SHALL derive their colors from one resolved palette.

#### Scenario: Theme changes once

- **WHEN** the user config selects a different theme
- **THEN** `Sheet` output and the TUI both reflect it without a second setting

### Requirement: Color suppression precedence

`--no-color`, `NO_COLOR`, and non-terminal output SHALL suppress color before any theme applies.

#### Scenario: Piped output with a theme set

- **WHEN** a command with a configured theme writes to a pipe
- **THEN** the output carries no color codes
