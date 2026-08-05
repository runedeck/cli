---
adr: "docs/decisions/CLI-0024 Interactive and Automated Tool Commands.md"
status: proposed
---
# Automated Tool Run

## Why

Automation needs a stable coding-tool command that reuses launch profiles and provider behavior without changing interactive launch or skill-script execution.

## What Changes

- Add `rune run [profile@]<tool>` for supervised noninteractive execution.
- Share launch resolution and provider adapters with existing commands.
- Add explicit sandbox modes, optional timeout, bounded output, and typed JSON failures.
- Move HarnessCouncil external provider calls to `rune run`.

## Capabilities

- run (new)
- launch (modified)
- bench (modified)

## Impact

- CLI command routing and help
- Shared provider and process lifecycle implementation
- Launch resolution and wrapper validation
- HarnessCouncil provider invocation instructions
- Manual testing and command documentation
