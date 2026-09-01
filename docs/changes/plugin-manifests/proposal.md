---
adr: "docs/decisions/CLI-0035 Plugin Manifests.md"
status: proposed
---

# Plugin Manifests

## Why

Git-style dispatch runs `rune-*` executables that declare nothing: no description, no listing,
no way to react to rune events. Governing decision: CLI-0035.

## What Changes

- Plugins live under `~/.config/rune/plugins/<name>/` with a `plugin.yaml` declaring `name`,
  `description`, `exec`, and `events`.
- `rune plugin list` shows every manifest.
- A successful install fires one `post-install` JSON event to subscribed plugins.
- Plugin failures warn once and never change the install result.

## Capabilities

- plugin (new)

## Impact

- One new plugin module, the install path, and the CLI dispatch
- CLI-0015 dispatch stays unchanged
