## 1. Implementation

- [ ] 1.1 Repeated `--capability` flags in `spec propose`; one delta spec per capability
- [ ] 1.2 Proposal template: `## Capabilities` section generated from the flags
- [ ] 1.3 `--design` flag scaffolding `design.md`; `spec context` includes it when present
- [ ] 1.4 `archive --abandon -y` accepted (no-op confirmation)
- [ ] 1.5 Smoke doc: note the OpenSpec root divergence (docs/ vs hardcoded openspec/) as intentional

## 2. Verification

- [ ] 2.1 Lifecycle smoke script runs verbatim including `--abandon -y`
- [ ] 2.2 A two-capability change round-trips: propose, context, archive
- [ ] 2.3 Existing single-capability changes scaffold unchanged (backward compatible)
