## Context

OpenSpec is the primary lifecycle engine, but its artifacts are Markdown that people and compatible tools can read without OpenSpec. The canon defines proposals, capability deltas, designs, and tasks, but no decision artifact that becomes a durable architecture decision record. Project-local workflow schemas can add that artifact.[OPENSCHEMA]

The Rune documentation lifecycle already provides ADR allocation, import, supersession, indexing, validation, and provenance. Decision publication connects that lifecycle to changes without using free-form proposal metadata as an implicit contract.

## Goals / Non-Goals

**Goals:**

- Define one decision-artifact shape for compatible interfaces.
- Publish accepted ADRs without duplicating authored rationale.
- Support atomic publication and idempotent reconciliation.
- Parse decision metadata into explicit ADR and exemption structures, rejecting malformed records and paths outside their declared roots.

**Non-Goals:**

- Select or route between command interfaces.
- Require one implementation of the OpenSpec artifact canon.
- Generate decisions from proposal or implementation prose.
- Publish historical decisions without review.

## Decisions

### Decisions as persistent and future-proof artifacts

Decision artifacts use Markdown as their canonical representation. Plain text remains readable without the authoring tool, works with version control, and keeps historical decisions independent of one command interface or storage format.

The project workflow adds `decisions/*.md` after proposal, specifications, and design. Tasks depend on the decision declaration, so implementation planning starts after decisions are reviewed.

Compatible interfaces read and write the same artifact shape. Compatibility concerns produced files, not command output or internal implementation.

Provider routing was rejected because it couples the canon to command behavior. Shared artifacts let each interface evolve independently.

### A change declares drafts or an exemption

Normal changes contain one or more canonical ADR drafts under `decisions/`. Maintenance changes contain only `decisions/no-decision.md`, with a typed reason explaining why no durable decision exists.

A scaffolder creates one draft by default. Authors may split independent decisions into separate files. OpenSpec status can detect the glob artifact, while strict validation enforces its content.[OPENSCHEMA]

Drafts already use the canonical ADR structure and `status: proposed`. Publication assigns the identifier, changes the status to `accepted`, updates publication metadata, and records the archived source. It preserves authored rationale rather than synthesizing it during archive.

### Publication supports two archive modes

A decision-aware archive validates and plans every write before mutation. Accepted ADRs, provenance, the decision index, specification updates, and the change move share one recoverable operation.

A preserving archive retains drafts inside the archived change. Reconciliation later publishes them in the same accepted shape. Health inspection reports pending drafts without writing.

Requiring one archive implementation was rejected because compatible interfaces may preserve the canon without owning its repository transaction system.

### Archived path and digest identify publication

Provenance records the archived source path and content digest. Reconciliation treats the same path and digest as already published and rejects a changed digest for a known path.

A valid no-decision marker requires no publication. A marker alongside drafts is invalid.

### Validation and allocation are shared

ADR drafts and no-decision markers deserialize into separate typed structures. Malformed YAML, missing values, invalid prefixes, unsupported statuses, mixed declarations, and unresolved placeholders fail validation.

The documentation lifecycle owns the templates and schemas. Compatible scaffolders use copies checked against those canonical assets, and native and schema validation share fixtures.

ADR creation, import, archive publication, and reconciliation coordinate allocation under one lock. Stable path ordering makes multi-draft allocation predictable, while exclusive destination creation prevents overwrite.

## Risks / Trade-offs

- Preserving archive leaves decisions unpublished until reconciliation; health output makes this state visible.
- Workflow-schema copies can drift from canonical templates; byte comparison makes drift detectable.
- Required decisions add ceremony to maintenance work; the reasoned exemption keeps omission explicit.
- Repository-scoped publication expands transaction recovery beyond specification-root writes.

## Migration Plan

- Add canonical ADR draft and no-decision contracts.
- Add the project workflow artifact and compatible scaffolding.
- Add read-only context, display, validation, and health support.
- Extend RuneSpec archive recovery to include decision publication.
- Add reconciliation for preserving archives.
- Adopt the workflow after existing active changes receive a reviewed draft or exemption.
- Review historical archives separately.

## References

- [OpenSpec custom workflow schemas][OPENSCHEMA]
- [OpenSpec archive workflow][OPENARCHIVE]

[OPENSCHEMA]: https://github.com/Fission-AI/OpenSpec/blob/main/docs/concepts.md "OpenSpec concepts"
[OPENARCHIVE]: https://github.com/Fission-AI/OpenSpec/blob/main/docs/opsx.md "OpenSpec workflow"
