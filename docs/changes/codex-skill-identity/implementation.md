# Worker instructions

Read the [requirements](specs/codex-skill-identity/spec.md), [ADR](adr.md), and [acceptance contract](acceptance.md).
The source base is `a9176a18009c4d1e3e979431f0ace761874f76e8`.
The user authorized implementation and two separate PRs on 2026-09-10.

## Fixed result

Use `ByKind` to place Codex skills in `.agents` and keep other kinds in `.codex`.
Preserve explicit target overrides and the six portable frontmatter fields.
Select one harness or model variant with the existing precedence and merge its top-level keys.
Preserve complete user-entrypoint replacement before that merge stage.
Use complete entrypoint replacement for procedure differences.
Keep shared instructions in companions. Qualifier folders do not deploy as extra skills.

Add the opt-in `rune validate --skill-layers --source <skill-folder|module|deck>` gate.
Keep it read-only and independent from ordinary validation compatibility.
Check generic and user layers against portable rules and harness layers against their own metadata and symbols.
Require explicit variant modes and exact configured model IDs under the correct provider.
Require complete user replacements with `mode: replace` and the canonical skill name.
Permit only `mode` in model-variant frontmatter to preserve identity, routing, and runtime metadata.
Check every authored layer and resolved output, including shared text companions and retained base text.
Policy inheritance does not make body inheritance cumulative.
Use one reviewed literal-symbol policy for source validation and rendered doctor findings.
Include quoted examples. Do not broadly prohibit provider names or `.codex` path mentions.
Report unknown layers, missing input, unreadable text, and zero checked skills as failures.
Do not suppress findings, skip malformed variants, or rewrite content automatically.
Static model checks prove declared constraints only. Native acceptance remains separate.
Permit empty metadata-only append/prepend bodies when the resolved entrypoint remains complete and nonempty.

Hash every bundle path, entry type, executable bit, file body, and symlink target.
Exclude generated provenance from the content digest and validate its claims separately.
Use the reserved `.provenance` namespace for build sidecars; preserve adjacent authored YAML as content.
Reject escaping links, missing companions, cycles, and case collisions.

Bind source selection, authored roots, effective configuration, builder bytes, and model overrides through the source
snapshot.
Doctor derives its inputs from current configuration and available pinned caches without fetching.
Publish a complete selected-source record only after a complete successful deployment.

Inventory declared names across applicable roots without selecting a duplicate winner.
Keep doctor read-only. Require a complete selected catalog and observed access through a harmless representative canary.
Reject absent, skipped, malformed, stale, disabled, or mismatched evidence.

Before migration writes, validate every legacy claim, complete bundle, and destination.
Preserve unknown ownership, local edits, untracked additions, and foreign destinations.
Quarantine whole verified legacy bundles only after replacement succeeds.
Preserve ordinary CLI-0003 writes outside legacy migration.
Refuse competing shared-root writers before target writes.

## Work boundaries

Assign each worker exact files. Keep acceptance fixtures under independent reviewer ownership.
Keep the architecture in the CLI PR and skill/rule portability corrections in the deck PR.
Use independent clones. Do not edit the active Claude run's files, plans, bookmarks, or task state.

A worker may change helper names and equivalent private implementation details.
A worker must not add block replacement, cumulative variants, a new ownership schema, or another deployment writer.
A worker must not change acceptance thresholds or label model self-report as tool evidence.
The content scope includes SafetyFirst, the five measured baseline skills, and their affected authoring guidance.
Do not suppress baseline failures or change unrelated Claude-owned ceremony, publication, or consumer-hook work.

## Verification

Run the focused integration tests before broad checks.
Exercise valid and invalid source-layer fixtures and rendered-bundle leakage controls.
Record the measured deck baseline separately from fixture results.
Inspect failure reasons and assertion counts. A zero exit code alone is insufficient.
Run format, Clippy, full tests, and the mandatory repository gates on the exact candidate.
Preserve raw native evidence outside the tracked source tree.
Return exact commands, counts, output digests, and unresolved checks to the independent reviewer.

Tests grant no publication authority. Publication in this run follows the user's explicit authorization.
The maintainer reviews the two PRs before merge or real deployment.
