## ADDED Requirements

### Requirement: Manifest listing

A plugin directory is a directory under `~/.config/rune/plugins/` that holds a `plugin.yaml` file, the manifest. The manifest declares a `name`, a `description`, an `exec` path, and the `events` the plugin subscribes to. `rune plugin list` MUST show every valid manifest with its name, its events, and its description, and MUST report each manifest that it cannot accept as invalid.

#### Scenario: Manifests list

- **WHEN** two plugin directories carry valid `plugin.yaml` files
- **THEN** `rune plugin list` shows both, each with its declared events and its description

### Requirement: Post-install event

When `rune install` or the TUI install deploys without an error, rune MUST run every plugin subscribed to `post-install` once and MUST write one JSON event to its standard input. The event MUST carry the event name, the resolved source path, the target, the requested providers, and the deployed count. A dry run and the staging install of `rune release` MUST NOT fire the event.

#### Scenario: Subscribed plugin receives the event

- **WHEN** an install succeeds and one plugin subscribes to `post-install`
- **THEN** that plugin runs once and its standard input carries the JSON event

### Requirement: Fault isolation

A plugin that exits nonzero, cannot start, or runs longer than 30 seconds MUST cause one warning that names the plugin. A plugin failure MUST NOT change the install result or its exit status.

#### Scenario: Failing plugin cannot break the install

- **WHEN** a subscribed plugin exits nonzero during an install
- **THEN** the install deploys its runes and exits 0, and one warning names the plugin

### Requirement: Executable confinement

A manifest's `exec` path MUST resolve inside its own plugin directory. Rune MUST first check the path by its components, so a missing target outside the directory still counts as an escape, and MUST then check the canonical path. A manifest that fails either check is invalid, and rune MUST NOT run its plugin.

#### Scenario: Escaping executable is rejected

- **WHEN** a manifest names an executable outside its plugin directory
- **THEN** `rune plugin list` reports the manifest as invalid because its `exec` escapes the directory, and an install never runs that executable
