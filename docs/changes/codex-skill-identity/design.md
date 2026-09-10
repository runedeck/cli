# Design for Codex skill identity

## Context

See [proposal](proposal.md) for scope and [requirements](specs/codex-skill-identity/spec.md) for observable behavior.
The owner selected a staged layout migration on 2026-09-10.
Use existing `ByKind` support. Defer cross-provider routing by assembled identity.
This PR uses an independent clone of the recorded CLI base.
The maintainer must reconcile overlapping changes from the separate Claude issue run before merge.

## Goals / Non-Goals

**Goals:**

- Verify source-to-bundle identity through the existing assembly and deployment pipeline.
- Detect ambiguity without modifying foreign user content.
- Preserve portable metadata and complete companions.
- Check generic, harness, and exact-model source layers and the rendered instructions they produce.
- Require separate native evidence before claiming discovery readiness.

**Non-Goals:**

- Another writer, orchestration service, or manifest ownership format.
- The deferred cross-provider equality router or changes to other providers' naming policy.
- Automatic user-wide deduplication, installation, or Codex configuration edits.
- Trigger-quality benchmarks or model-quality claims.
- Changing ordinary install conflict behavior, cast coverage, or the self-skill installer.

## Decisions

The [draft ADR](adr.md) fixes the architecture and alternatives.
The following details constrain implementation within that decision.

### Reuse placement and inspection

Change Codex's skills target through `ProviderTarget::ByKind` and keep its private fallback.
Resolve CLI provider arguments by exact key, then explicit alias, then a unique target-directory match.
Reject ambiguous CLI matches with sorted candidates before assembly or deployment.
Use the same resolved provider names for assembly, source records, and deployment.
Per-rune `targets` are content selectors and keep every matching provider through `ProviderConfig::matches_target`.
They accept configured names, aliases, and target directories without using the unique CLI resolver.
Add `doctor --skill-readiness` as a read-only mode. It conflicts with `--repair`.
Implement the new checks in a read-only helper shared by that inspection and its acceptance tests.
Use `src/skill_readiness.rs` for inventory and `src/cli/doctor/skills.rs` for CLI context.

Keep existing installed, stale, and missing listing meanings.
Listing proves manifest integrity only. It does not prove source freshness or native discovery.
Expose additive `skill_readiness` data in the doctor JSON report.
Keep existing report fields and stdout behavior intact.

The additive data records `cwd`, `repository_boundary`, `roots`, `skills`, and `findings`.
It separates `static_valid`, `native_discovery`, and `accepted`.
Each skill records source identity, source revision or content digest, authored path, declared name, target path, and
bundle digest.
Each finding records a stable code, affected selected identity, candidate paths, and ownership evidence state.
Use the finding codes defined in [acceptance](acceptance.md). Sort skills and findings deterministically.
When selection is unavailable, retain ordinary inventory output and mark readiness incomplete.
Do not claim to infer selected source identities from installed names alone.

### Keep the scope explicit

Inventory applicable user and repository roots, including ancestor roots within the actual repository boundary.
Include configured skill roots and legacy Codex roots in the static candidate report.
Record admin, system, plugin, disabled-skill, and harness-specific scope through supported harness evidence when
applicable.
Do not assume every legacy candidate is loaded by the current harness.
Do not assume the four roots from the original audit are the complete scope.

Parse each entrypoint with the existing frontmatter parser.
Compare declared names case-sensitively. Check case aliases separately when resolving filesystem paths.
Canonicalize root aliases for filesystem traversal, but retain distinct harness entries until native evidence proves
their discovery identity.
Never deduplicate two separate named entries merely because their bundle digests match.

### Hash the assembled bundle

Reuse existing file enumeration and digest facilities.
Create a sorted record for each deployed relative path, entry type, executable bit, symlink target, and file digest.
Hash an unambiguous serialization with a format version, `rune-skill-bundle/v1`.
Exclude `.provenance/` content and timestamps. Validate provenance separately against the manifest.
Allow relative symlinks only when their resolved target stays inside the bundle and has no cycle.
Reject case collisions on the target filesystem. Do not hash a directory listing's incidental order.

Use existing Markdown/reference parsing for documented companion links.
Do not treat every prose filename or code example as a required file.
Test executable scripts and their supported sibling imports with representative fixtures.
This check does not claim static resolution of arbitrary runtime file access.

### Separate source and build metadata

Generated build sidecars use the reserved `.provenance` directory, as deployed sidecars already do.
Adjacent generated YAML can collide with authored YAML and cannot be classified safely from its contents.
Do not exclude authored data because it resembles a provenance statement.

A versioned source snapshot covers caller-selected configuration and authored source roots.
The snapshot includes new and removed files, file modes, and contained symlink targets.
It binds raw selection files, effective embedded configuration, and the current builder executable bytes.
Explicit model overrides are recorded; doctor requires a matching `--skill-model` value.
Inputs cover every configured local source or already-materialized pinned Git source.
Changes to unselected content can therefore invalidate readiness conservatively.
Generated build, provider, and documentation roots are not snapshot inputs.
It binds the complete selected build skill map to the source state.
Deployment records it only after a complete successful operation.
Partial or unpruned operations cannot publish a complete source snapshot.
Doctor derives allowed roots from current configuration and cached sources, then computes the snapshot without fetching.
Doctor never traverses arbitrary source paths supplied by the recorded snapshot.
Unavailable, stale, partial, or mismatched snapshots prevent acceptance.

### Preserve metadata through the existing filter

Extend Codex's retained skill fields to `name`, `description`, `version`, `license`, `compatibility`, and `metadata`.
Compare those fields with the resolved frontmatter after variant merging, not the unqualified base frontmatter.
Preserve the resulting nested metadata as parsed values. Keep existing authoring directive removal.
Validate frontmatter placement without claiming that retained metadata constrains harness permissions.
Preserve `agents/openai.yaml` as a companion when authored. Do not generate one for every skill.

### Store variant text in existing qualifier folders

Use the existing source layout inside the canonical skill directory:

```text
ExampleSkill/
  SKILL.md
  Workflow.md
  codex/
    SKILL.md
  claude/
    SKILL.md
```

`Workflow.md` contains portable shared instructions when that extraction improves clarity.
Each short entrypoint references `Workflow.md` at its assembled relative path.
Provider-specific text lives in the matching qualifier's `SKILL.md`.
This is an authoring pattern for fixtures and future content changes, not a bulk rewrite of existing skills.

For a changed procedure, use explicit `mode: replace` and supply the complete entrypoint body.
The shared workflow remains in its companion file, so the entrypoints need not duplicate that workflow.
Use explicit `mode: append` only for additive text or metadata-only variants that preserve the base body.
Append does not override earlier instructions. It concatenates bodies.
`mode: prepend` also remains supported. It places the variant body before the base body.
Do not add heading matching, block identifiers, text patches, or model-directed merge decisions.

The collector first replaces the complete entrypoint with `user/SKILL.md` when that file exists.
This user entrypoint does not merge with the canonical base.
Without a user replacement, the resolver selects `provider/model/`, then `provider/`, then the base file.
The selected harness or model variant combines with the base.
These qualifier levels do not form a cumulative overlay chain.
A model qualifier must use an exact configured model identifier.

Variant frontmatter keys replace matching base keys in full, including nested maps.
The body mode does not change that frontmatter rule.
An omitted mode currently defaults to `replace`. Preserve that compatibility behavior and make modes explicit in new
fixtures. The opt-in source-layer gate rejects omitted modes without changing ordinary assembly defaults.

Assembly emits one resolved `SKILL.md` and its selected companions at the configured target.
The skill's qualifier folders are not copied as extra discoverable skills.
Shared versus private target routing remains the separate staged layout decision.

Current non-entrypoint companions bypass variant merging in the assembly pipeline.
Do not assume that `codex/Workflow.md` replaces `Workflow.md`.
Existing `user/` companion overrides remain direct file replacements during collection, without body merging.
Effective source checks use that same relative-path replacement, while portable checks still inspect both authored files.
Provider/model companion merging or block replacement would require a separate change.
See [variant resolution](../../../src/assemble/variants.rs) and [companion
passthrough](../../../src/cli/assemble/pipeline.rs).

### Validate source layers and resolved instructions

Add `rune validate --skill-layers --source <skill-folder|module|deck>` as a strict standalone read-only gate.
It accepts a canonical skill, module, or deck source root and uses the configured provider/model map.
It does not install, rewrite source files, fetch missing sources, invoke a model, or change ordinary validation behavior.
Malformed input, missing required input, unsupported scope, and zero checked skills cannot become a passing result.

Check these policy layers separately:

| Layer | Required policy |
| --- | --- |
| Generic entrypoint and shared companions | Permit portable Agent Skills metadata and existing source routing and invocation controls. Reject other enumerated harness runtime metadata and harness-only symbols. |
| Harness variant | Require a supported provider folder and explicit mode. Permit its own declared metadata and symbols, and reject foreign harness symbols and metadata. |
| Model variant | Require an exact configured model ID beneath its own provider. Require explicit mode and permit no other model frontmatter keys. Apply that harness's symbol rules. |
| Resolved source and rendered bundle | Reuse the actual single-winner merge contract for source checks. Check actual rendered output separately through doctor after provider transformation and filtering. |

The source gate also checks generic `user/` overrides under the portable policy.
It requires `user/SKILL.md` to declare `mode: replace`, complete `name` and `description`, and a nonempty body.
The user entrypoint must preserve the canonical skill name.
The selected base or complete user entrypoint supplies provider applicability through `targets`.
An authored target list must be nonempty and match configured provider names, aliases, or target directories.
Load those provider definitions from the selected source configuration through the same deep merge as assembly.
Malformed configuration fails inspection without a compatibility fallback to embedded defaults.
An unavailable source returns the versioned report with `SL000_INVALID_INPUT` and the requested path.
A model variant cannot change skill identity, source routing, or runtime metadata.
Generic source can retain `targets`, `disable-model-invocation`, and `user-invocable` as existing assembly controls.
Their source placement does not authorize leaking invocation directives into rendered Codex frontmatter.
Model checks verify declared source constraints. They do not establish model behavior, tool availability, or model quality.
Unknown providers, unknown model IDs, and model folders under the wrong provider produce explicit findings.
Do not silently discard a malformed or unsupported variant during validation.

Policy inheritance does not change body inheritance.
The generic, harness, and model lint policies combine where applicable.
The resolver still combines one winning harness or model variant with the base entrypoint.
The complete user replacement remains the collection-stage exception.
A model variant does not inherit the provider variant's body.
Check every authored variant even when another variant wins the current render.
Check the merged body again so an append or prepend cannot retain forbidden base instructions unnoticed.
Metadata-only harness or model append/prepend variants may have empty bodies when the resolved body remains nonempty.

Scan the entrypoint and text companions, including examples, quotations, and conditional instructions.
The policy matches a reviewed finite set of literal harness symbols.
Its frontmatter restrictions are also bounded. They do not implement a universal schema for every permitted source key.
It does not ban provider names, `.codex` path mentions, or every term that might describe a tool.
Do not add prose-based exemptions, suppression comments, or automatic rewrites.
This literal check cannot detect every semantic dependency or prove that arbitrary runtime file access is portable.
Binary assets remain bundle inputs. Invalid encoding in an instruction file is an explicit validation failure.
Preserve contained-link validation and do not use source aliases to read outside the permitted root.

Doctor applies the Codex policy to the inspected rendered bundle and reports `CSI007_HARNESS_LEAKAGE`.
The source gate inspects merged source before provider transformation and frontmatter filtering.
Doctor scans the actual entrypoint and emitted text companions after those assembly steps.
A filtered source-only key may disappear from output, but its invalid source placement still fails the source gate.
Thus a clean render cannot conceal an invalid authored layer, and clean source layers cannot replace rendered checks.

The measured deck baseline contains provider-dependent instructions in generic entrypoints and companions.
The expanded content PR resolves the measured baseline alongside SafetyFirst and the affected authoring guidance.
Record its new result without suppressions. Keep unrelated work from the active Claude run outside this change.

### Reconcile canonical guidance without changing approval state

The accepted ASSEMBLY-0001 and ASSEMBLY-0004 records retain their original design text and acceptance status.
Dated implementation notes identify the skill user-replacement exception and current build/deployment provenance paths.
Repository agent guidance uses the current nested model layout and distinguishes skill collection from variant merging.
These corrections do not accept the unnumbered draft ADR or promote this change into canonical specifications.

### Keep migration narrower than ordinary installation

Use manifest fingerprints and valid source provenance to establish migration ownership.
Check every tracked companion and every untracked addition before moving a complete directory.
An occupied foreign destination becomes a migration conflict before the existing migration write path runs.
The refusal uses `CSI005_MIGRATION_CONFLICT` and preserves the affected content.
Ordinary install behavior under CLI-0003 remains unchanged.
The migration does not invoke generic orphan repair to clean a legacy root.
Keep previous content recoverable until replacement and claim updates succeed.
Back up only replacement bundles that passed preflight ownership validation.
On failure, retain partial content in a reported recovery directory before restoring the previous destination.
Restore claims only for the affected skill prefixes and preserve unrelated claims.
Check that the physical target root remains unchanged before recovery writes.
Refuse symlinked replacement roots instead of restoring through them.

### Separate deterministic checks from native discovery

Static CI validates the fixtures without a model or personal files.
A later fresh-session acceptance run proves the supported harness discovers and accesses the intended bundle.
Record that observation separately from the doctor report. Do not make doctor launch a model.
The adapter records the complete catalog before doctor computes its inventory digest.
The evidence binds normalized events to exact raw protocol records, including their indices and SHA-256 digests.
Require a refreshed `skills/list` response, a fresh read-only session, explicit skill input, and a successful
companion read.
Compare every selected skill with the complete catalog. Invoke only a harmless representative canary for the
companion-access control.
Reject model prose, failed commands, incomplete catalogs, and records older than 24 hours.
The producer's identity remains an external trust boundary. Hashes do not authenticate a malicious evidence producer.
Missing native evidence leaves readiness unverified while preserving the useful static result.
Explicit invocation tests source identity. Implicit triggering quality belongs to improvement 7.

## Risks / Trade-offs

- A complete static inventory can miss harness-specific sources. Require recorded native scope and keep unknown scope
  visible.
- Doctor can return zero for modified files. The acceptance checker evaluates structured findings as well as the exit
  status.
- Existing shared-root writers can conflict. The stage-one test must expose this without introducing the deferred
  owner-schema design.
- Concurrent PRs can change the same defaults. Reconcile their behavior and tests before merge.
- Source and executable bytes must remain stable during validation. Concurrent rebuilds can invalidate snapshot
  evidence.
- The native evidence adapter may lack a supported catalog surface. Record unverified evidence and keep that
  acceptance task incomplete.

## Migration Plan

1. Prepare and validate the architecture in an independent CLI clone.
2. Verify complete bundles and migration recovery in disposable targets.
3. Run native discovery against disposable contexts and preserve raw evidence.
4. Review the architecture PR separately from the skill and rule portability PR.
5. Reconcile overlapping changes from the active Claude issue run before merge.
6. Retain quarantine content and previous claims needed for recovery.

The user authorized PR publication. This change does not deploy into real provider directories.
