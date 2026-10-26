# Plugin manifests design

## Approach

A manifest with one event sits between bare dispatch and a plugin marketplace. The manifest lets `rune plugin list` show a plugin before anything runs it. The `post-install` event serves the jobs that run after a deploy, such as a sync, a notification, or an index rebuild, and a plugin needs nothing beyond its standard input to read it.

## Structure

- `src/cli/plugin.rs` reads each `plugin.yaml` under `~/.config/rune/plugins/`, prints `rune plugin list`, and runs the subscribed plugins for an event.
- After a deploy without an error, the install path builds one JSON payload: the event name, the resolved source, the target, the requested providers, and the deployed count.
- The runner writes the payload to the plugin's standard input, starts the plugin in its own directory, and discards its output.
- The `exec` check reads the path components first, so a missing target outside the directory still reports the escape. The runner then canonicalizes the path and requires it inside the plugin directory. A manifest that fails either check is invalid, and its plugin never runs.

## Risks

- Plugins run with the user's permissions. The listing shows local executables, and a listing is not an endorsement.
- A plugin that hangs would stall the end of an install. The runner kills a plugin after 30 seconds and prints a warning.
- The runner knows one event. A larger event set needs its own decision.
