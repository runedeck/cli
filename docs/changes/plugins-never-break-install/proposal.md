---
adr: "docs/decisions/CLI-0035 Plugin Manifests.md"
status: proposed
decisions: ["CLI-0035 Plugin Manifests"]
---

# Plugin manifests

## Why

Git-style dispatch (CLI-0015) runs any `rune-*` executable on `PATH`, and such a program declares nothing. Rune cannot list it with a purpose before it runs, and the program cannot react to a rune event, so a deploy hook needs a wrapper script around `rune install`. [Herdr][HERDR] gives each plugin a manifest that is "the contract between Herdr and the plugin" and declares its actions and event hooks. Rune needs the smallest form of that contract: a declaration that a listing can show, and one event after a deploy, where sync, notification, and index jobs start.

## What Changes

- A *plugin directory* under `~/.config/rune/plugins/<name>/` holds a `plugin.yaml` *manifest* with `name`, `description`, `exec`, and `events`.
- `rune plugin list` shows every valid manifest with its events and description, and reports each invalid manifest. `--json` prints both lists.
- `rune install` writes one `post-install` JSON event to the standard input of each subscribed plugin after it deploys. A dry run fires nothing.
- A plugin that fails or runs longer than 30 seconds causes one warning and never changes the install result.
- A manifest whose `exec` resolves outside its plugin directory is invalid, and its plugin never runs.

## Capabilities

- plugins-never-break-install (new)

## Impact

- `src/cli/plugin.rs` holds discovery, the listing, and the event runner.
- `src/cli/install/mod.rs` fires the event, and `src/cli/mod.rs` adds `rune plugin list`.
- `tests/plugin.rs` covers the listing, the event payload, fault isolation, and confinement.
- The `rune-*` dispatch of CLI-0015 keeps its behavior.

[HERDR]: https://herdr.dev/docs/plugins/
