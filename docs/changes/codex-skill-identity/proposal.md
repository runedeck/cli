# One identifiable Codex skill copy

## Why

Duplicate skill names can expose different instructions to the same Codex session.
Moving skills to `.agents` needs evidence that the selected source, complete bundle, and discovered path agree.

## What Changes

- Implement Codex placement, complete-bundle identity, safe migration, and strict readiness checks.
- Record selected skill identity, deployment path, complete bundle digest, and ownership evidence.
- Bind current source selection, authored files, effective configuration, and builder bytes through a read-only source
  snapshot.
- Report duplicate declared names across the effective discovery scope without guessing which copy wins.
- Preserve portable metadata and companion files through assembly and migration.
- Add a strict read-only gate for generic, harness, and exact-model source layers and their resolved output.
- Check rendered Codex entrypoints and text companions for explicit foreign harness symbols and runtime metadata.
- Reserve `.provenance` for generated build metadata so authored YAML remains content.
- Quarantine only obsolete copies with proven Rune ownership through the existing migration path.
- Distinguish filesystem integrity from observed discovery in a fresh Codex session.
- Add parser-based checks, regression fixtures, and a separately recorded native discovery check.
- **BREAKING:** the default Codex skill destination becomes `.agents/skills`.

## Capabilities

### New Capabilities

- `codex-skill-identity`: Verify selected skill identity, layer validity, bundle integrity, ambiguity, and observed discovery.

### Modified Capabilities

The existing assembly, deployment, and doctor interfaces gain the behaviors specified by this capability.
`rune validate --skill-layers --source <skill-folder|module|deck>` adds an opt-in standalone source gate.
Ordinary validation keeps its existing behavior.

## Impact

The target implementation repository is the Rune CLI.
The existing `ByKind` placement, assembly, manifests, migration, doctor findings, and skill listing provide the
implementation surfaces.
No new deployment service, cross-provider equality router, global cleanup process, or manifest-owner schema is proposed.
The deck supplies representative source fixtures. This change does not rewrite its skills or change its casts.

The user authorized this isolated implementation through a PR on 2026-09-10.
The CLI base is `a9176a18009c4d1e3e979431f0ace761874f76e8`.
This change overlaps the queued Codex layout work in the separate Claude issue run.
Reconcile that overlap before merge. Do not modify that run's working files or decisions from this PR.
Keep the ADR unnumbered until integration to avoid reserved decision numbers.

The [ADR](adr.md), [worker instructions](implementation.md), and [acceptance contract](acceptance.md) define this
implementation.
The companion deck PR corrects SafetyFirst and the measured portability baseline in five other skills.
It also aligns the affected skill-authoring guidance and source rules with this contract.
Keep unrelated Claude-owned ceremony, schema-publication, and consumer-hook work outside these PRs.
The gate does not rewrite skills, suppress findings, or prove model behavior from declared constraints.
Routing by assembled identity remains deferred.

Codex keeps duplicate names as separate skills. See [official skill
discovery](https://learn.chatgpt.com/docs/build-skills).
