## ADDED Requirements

### Requirement: Theme-derived TUI palette

The TUI SHALL derive every color from the resolved theme, and a light theme SHALL use light
surfaces with dark text.

#### Scenario: Light theme selected

- **WHEN** the user config selects a light theme
- **THEN** the TUI panels, status bar, and text use the light palette

### Requirement: Status bar context

The TUI status bar SHALL show the deck name, the bound target when one exists, and one glyph per
enabled provider that reflects its deployment state.

#### Scenario: Provider needs repair

- **WHEN** a provider's deployment state is `needs repair`
- **THEN** the status bar shows that provider with the repair glyph in the bad tone

### Requirement: First-run panel

When the scan finds no deck and no modules, the TUI SHALL replace the list with a panel that names
the root and the commands that configure a deck, and the footer SHALL point at `rune setup`.

#### Scenario: TUI opens on an unconfigured root

- **WHEN** `rune tui` starts in a directory with no deck and no modules
- **THEN** the panel names `rune setup`, `rune config set deck`, and `rune tui --source`

### Requirement: Help overlay close keys

The help overlay SHALL name its close keys and the rune version in its title.

#### Scenario: User opens help

- **WHEN** the user presses `?`
- **THEN** the overlay title names `?` and `Esc` as the close keys
