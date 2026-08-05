## Why

OpenSpec changes record design rationale but do not produce durable [Architectural Decision Records (ADRs)][ADR]. A shared decision-artifact contract lets compatible interfaces publish the same accepted record.

## What Changes

- Add canonical ADR drafts under `decisions/*.md`, with an exclusive `decisions/no-decision.md` exemption.
- Define typed validation and lifecycle visibility for decision declarations.
- Publish accepted ADRs during decision-aware archive or through reconciliation after preserving archive.
- Keep the artifact process independent of the interface used to manage a change.

## Capabilities

### New Capabilities

- `decision-artifacts`: Defines decision declarations, publication, reconciliation, validation, and provenance within the OpenSpec canon.

### Modified Capabilities

None.

## Impact

- Extends the change tree and project workflow schema with decision declarations.
- Adds canonical ADR draft and no-decision contracts.
- Extends compatible lifecycle, context, validation, and health surfaces.

[ADR]: https://adr.github.io/ "Architectural Decision Records"
