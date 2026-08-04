---
status: proposed
---
# OpenSpec Root Selection

## Why

Repositories with an `openspec/` tree need a durable choice between direct operation and conversion to the native documentation root. Automated commands need the same root resolution without an interactive prompt.

## What Changes

- The first interactive lifecycle command on an unconfigured OpenSpec root offers to keep `openspec/` or migrate to `docs/`, then records the explicit answer as `spec.root`.
- Automated and JSON commands use the autodetected root, write no configuration, and print one advisory note.
- Import and export bypass the offer because they perform the conversion directly.
- Doctor attempts the optional upstream OpenSpec validator and reports failures as advisory warnings.

## Capabilities

- spec-lifecycle (modified)

## Impact

- CLI root selection and configuration writes
- OpenSpec advisory validation
- Spec walkthrough and manual testing
