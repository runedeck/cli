## ADDED Requirements

### Requirement: Manifest listing

`rune plugin list` SHALL list every plugin manifest under the plugin directory with its name,
description, and subscribed events.

#### Scenario: Manifests list

- **WHEN** two plugin directories carry valid `plugin.yaml` files
- **THEN** `rune plugin list` shows both with their declared events

### Requirement: Post-install event

After a successful install, rune SHALL run every plugin subscribed to `post-install` with one
JSON event on stdin carrying the source, the target, the providers, and the deployed count.

#### Scenario: Subscribed plugin receives the event

- **WHEN** an install succeeds and one plugin subscribes to `post-install`
- **THEN** that plugin runs once and its stdin carries the JSON event

### Requirement: Fault isolation

A plugin failure SHALL print one warning and SHALL never change the install result.

#### Scenario: Failing plugin cannot break the install

- **WHEN** a subscribed plugin exits nonzero
- **THEN** the install result stays unchanged and one warning names the plugin

### Requirement: Executable confinement

A manifest's executable SHALL resolve inside that plugin's own directory.

#### Scenario: Escaping executable is rejected

- **WHEN** a manifest names an executable outside its plugin directory
- **THEN** the plugin is rejected with a structured error and does not run
