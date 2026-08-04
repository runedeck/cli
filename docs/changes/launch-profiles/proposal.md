---
adr: "docs/decisions/CLI-0021 Launch Profile Composition.md"
status: proposed
---
# Launch Profiles

## Why

Coding-tool profiles need named endpoint, credential-reference, argument, middleware, and model-route settings without replacing the accepted launch middleware chain.

## What Changes

- `rune launch [profile@]<tool>` resolves named profiles from user configuration.
- Profiles compose environment, arguments, middleware, and one optional model route.
- Model routes derive provider model and context settings as one group.
- Interactive execution keeps inherited terminal input and output plus native argument forwarding.

## Capabilities

- launch (modified)

## Impact

- Launch configuration and typed ontology
- Launch resolution, dry-run output, redaction, and interactive execution
- Automated execution that consumes the shared resolved launch plan
