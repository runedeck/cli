# Plugin Manifests Design

## Approach

A manifest plus one event beat bare dispatch and a premature marketplace. The manifest makes a
plugin visible before anyone runs it, and the single `post-install` event covers the dominant
automation case (sync, notify, index) without a new API surface.

## Structure

- `src/cli/plugin.rs`: manifest discovery and parsing, `rune plugin list`, and the event
  runner.
- The install path calls the event runner after a successful deploy with one JSON payload:
  source, target, providers, deployed count.
- The executable path canonicalizes inside the plugin directory; anything else is rejected
  with a structured error.

## Risks

- Plugins run with the user's permissions; the listing labels them as local executables, not
  endorsements.
- A hanging plugin would stall the install tail; the runner bounds each plugin's run time.
- One event only; a richer event set needs its own change.
