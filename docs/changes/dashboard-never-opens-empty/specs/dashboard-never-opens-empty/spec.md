## ADDED Requirements

### Requirement: Theme-derived TUI palette

The TUI MUST derive its palette from the resolved theme. The palette MUST cover panels, borders,
the status bar, interface text, diffs, and state tones, and a light theme MUST give it light
backgrounds with dark text.

#### Scenario: Light theme selected

- **WHEN** the user config selects a light theme
- **THEN** the palette gives the panels and the status bar light backgrounds and dark text

### Requirement: Status bar context

The TUI status bar MUST show the deck name, the bound target when one exists, and one glyph per
enabled provider that reflects its deployment state.

#### Scenario: Provider needs repair

- **WHEN** provider detection classifies a provider as `needs repair`
- **THEN** the status bar shows `✗` after the name of that provider

### Requirement: First-run panel

When the scan finds no deck and no modules, the TUI MUST replace the list with a panel that names
the root and the commands that configure a deck, and the footer MUST point at `rune setup`. When
the scan fails, the footer MUST name the error and the panel MUST stay closed.

#### Scenario: TUI opens on an unconfigured root

- **WHEN** `rune tui` starts in a directory with no deck and no modules
- **THEN** the panel names `rune setup`, `rune config set deck`, and `rune tui --source`

#### Scenario: Scan fails on a root without providers

- **WHEN** `rune tui` starts in a directory with no deck and no deployed provider directory
- **THEN** the footer names the missing provider directories, and the panel stays closed

### Requirement: Help overlay close keys

The help overlay MUST name its close keys and the rune version in its title.

#### Scenario: User opens help

- **WHEN** the user presses `?`
- **THEN** the overlay title names `?` and `Esc` as the close keys
