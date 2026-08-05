## Context

See `proposal.md` for motivation and `specs/development-lifecycle/spec.md` for observable behavior.

Rune already has independent specification interfaces, decision artifacts, implementation planning practices, test commands, staged review, pull-request checks, and publication mechanisms. They do not yet form one resumable process. The deck's `development` domain is reserved for review flow, verification, commit conventions, and delivery discipline, so it owns the coordinating skill.

## Goals / Non-Goals

**Goals:**

- Coordinate existing lifecycle functions without replacing their enforcement.
- Keep the interview portable across interactive AI harnesses.
- Preserve explicit user decisions at phase transitions.
- Resume from repository and platform state rather than conversational memory.

**Non-Goals:**

- Route RuneSpec commands through OpenSpec or OpenSpec commands through RuneSpec.
- Replace specialized specification, ADR, testing, review, or deployment functions.
- Infer approval from validation success, a created pull request, or an earlier approval.
- Make unattended merge or deployment the default.

## Decisions

### DevelopmentLifecycle coordinates the process

The canonical skill is `DevelopmentLifecycle` under the deck's `development` domain. It inspects the active change, identifies the earliest incomplete phase, and invokes the appropriate native function for that phase.

The skill coordinates judgment and sequencing. Rune, OpenSpec, version control, continuous integration, and deployment tools remain the enforcement boundaries for their own artifacts and operations.

A monolithic replacement command was rejected because it would duplicate those systems and couple the lifecycle to one interface.

### The specification interview is harness-portable

The canonical skill describes a structured question capability rather than naming a provider tool. Each harness uses its corresponding structured question function. When none exists, the skill asks the same bounded question in plain text and waits.

The interview runs in the user-facing context. It first reviews goals and non-goals, then walks authored scenarios with questions derived from their actual triggers, outcomes, and boundaries. Generic approval questions do not satisfy the review.

After eight questions, unresolved scenarios cause a continuation choice. The user may continue individually, review the remainder as a group, or stop for manual editing. Eight is a pacing boundary, not a silent coverage limit.

Naming one harness's question function in the canonical skill was rejected because the deck compiles the same behavior for several harnesses.

### Interview changes apply immediately

An adjustment edits the specification, reruns strict validation, and re-presents only the affected material. The interview remains active until the revised content validates and the user accepts it.

Collecting notes for a later edit pass was rejected because review comments can drift from the artifact they describe and leave acceptance ambiguous.

### Tasks record phase acceptance

`tasks.md` carries explicit lifecycle checkpoints, including completion of the specification interview, decision review, implementation-plan review, verification, staged review, pull-request review, and deployment.

The lifecycle checks a task only after the corresponding user approval or verified outcome. If the lifecycle edits an accepted phase, it clears that phase and every dependent checkpoint before continuing.

No separate review ledger is introduced. This keeps the workflow readable in plain Markdown, but a manual edit outside the lifecycle can leave a stale checkbox. Validation and review must treat changed accepted artifacts as requiring renewed approval.

A dedicated review artifact was rejected in favor of the lighter tasks checkpoint selected for this process.

### Phase transitions preserve approval boundaries

The lifecycle order is:

```text
specification
    -> tailored specification review
    -> decision artifacts
    -> implementation plan
    -> implementation and testing
    -> staged review and pull request
    -> deployment
```

Validation success permits an approval question; it never answers one. Plan approval does not authorize implementation publication, pull-request creation does not authorize merge, and merge approval does not authorize a separate release or installation.

A decision that changes accepted behavior returns the lifecycle to specification review. A code or test finding returns it to implementation. Pull-request feedback returns it to the affected phase rather than being patched outside the artifact chain.

### Deployment means published availability

Deployment is the point at which the reviewed result reaches its intended destination. For software this may be a release or installation. For documentation it is the merge that makes the files visible in the target GitHub repository.

The skill verifies destination-specific evidence before checking deployment complete. Outward-facing or hard-to-reverse actions require approval at that point.

### Resume derives state from durable systems

The skill reconstructs progress from OpenSpec artifacts, lifecycle checkboxes, decision records, version-control state, test evidence, pull-request state, and deployment evidence. It does not rely on one harness transcript.

On resume, it validates completed phases in order and starts at the earliest incomplete or invalid phase. It reuses accepted artifacts rather than recreating them.

## Risks / Trade-offs

- A tasks checkbox does not bind approval to a content digest. Editing tools clear dependent checkpoints; manual edits require review discipline.
- A cross-harness interview cannot guarantee identical presentation. The behavioral contract requires equivalent questions and outcomes, not identical UI.
- One coordinating skill can grow too broad. Phase mechanics stay in focused skills and commands, with this skill limited to routing and approvals.
- Deployment evidence differs by destination. Each integration defines what published availability means and how it is verified.

## Migration Plan

- Accept and implement the decision-artifacts capability as the specification-to-ADR boundary.
- Add lifecycle checkpoint sections to the project tasks template and workflow schema.
- Author `DevelopmentLifecycle` in the deck's `development` domain with portable question guidance.
- Connect the skill to existing specification, decision, planning, testing, review, and deployment functions without wrapping one specification CLI in the other.
- Add fixtures for accepted, revised, interrupted, review-returned, and deployed changes.
- Exercise the lifecycle on a documentation-only change and a software change before making it a default deck capability.
