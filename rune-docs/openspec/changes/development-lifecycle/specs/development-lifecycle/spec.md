## Purpose

Defines the portable, review-driven development lifecycle that carries a Rune change from specification through decisions, implementation, testing, review, and deployment.

## ADDED Requirements

### Requirement: Changes advance through reviewed lifecycle phases

A change MUST advance through specification, tailored specification review, decision declaration, implementation planning, implementation and testing, pull-request review, and deployment. A later phase MUST NOT begin until required artifacts validate and the user explicitly approves the preceding consequential transition.

#### Scenario: Change advances from specification to decisions

- **WHEN** the specification passes strict validation and the tailored review is accepted
- **THEN** the lifecycle permits decision drafting and preserves the accepted specification as its input

#### Scenario: Required phase is incomplete

- **WHEN** a required artifact is missing, invalid, or awaiting user approval
- **THEN** the lifecycle identifies the incomplete phase and does not advance to a dependent phase

### Requirement: Specification review uses tailored questions

After strict specification validation, the lifecycle MUST review the stated goals, non-goals, and each requirement scenario with questions tailored to their authored content. It MUST use the harness's structured question function when available. A harness without that function MUST ask the same bounded question in plain text and wait for the answer.

The interview MUST run in a context that can communicate directly with the user. Custom answers are clarification until they resolve to acceptance or a concrete revision.

#### Scenario: Harness supports structured questions

- **WHEN** the review runs in a harness with a structured question function
- **THEN** goals, non-goals, and scenarios are presented through that function with content-specific choices and custom clarification

#### Scenario: Harness lacks structured questions

- **WHEN** the review runs in a harness without a structured question function
- **THEN** it presents the tailored question in plain text and pauses until the user resolves it

#### Scenario: Review reaches its normal boundary

- **WHEN** eight questions have been asked and unresolved scenarios remain
- **THEN** the lifecycle asks whether to continue individually, review the remainder as a group, or stop for manual editing

### Requirement: Review revisions are applied and revalidated

When an answer changes a goal, non-goal, or scenario, the lifecycle MUST edit the specification immediately, rerun strict validation, and re-present the affected material. It MUST NOT treat the earlier wording as accepted after revision.

#### Scenario: User adjusts a scenario

- **WHEN** the user changes a scenario trigger, expected result, or boundary
- **THEN** the lifecycle updates the specification, validates it, and asks for acceptance of the revised scenario

#### Scenario: Revision breaks validation

- **WHEN** an interview revision produces an invalid specification
- **THEN** the lifecycle reports the validation finding and keeps specification review active

### Requirement: Decisions and implementation require approval

Decision artifacts MUST follow the accepted specification. Implementation MUST begin only after the decision declaration and implementation plan validate and receive explicit user approval.

#### Scenario: Decision changes the accepted behavior

- **WHEN** a decision draft conflicts with an accepted requirement or scenario
- **THEN** the lifecycle returns to specification review instead of allowing implementation planning to continue

#### Scenario: Implementation plan is accepted

- **WHEN** decision artifacts and the implementation plan validate and the user approves the plan
- **THEN** the lifecycle permits implementation work against those artifacts

### Requirement: Testing and review provide completion evidence

Implementation MUST produce test or verification evidence appropriate to the changed behavior. Changes MUST be staged for user review before commit, and publication through a pull request MUST retain its required review and continuous-integration checks.

#### Scenario: Verification fails

- **WHEN** required tests, validation, or continuous-integration checks fail
- **THEN** the lifecycle remains in implementation or review and reports the failing evidence

#### Scenario: Change is ready for pull-request review

- **WHEN** implementation and verification pass and the user approves the staged changes
- **THEN** the lifecycle permits pull-request creation without treating creation as merge approval

### Requirement: Deployment publishes the reviewed result

Deployment MUST place the reviewed change at its intended published destination and MUST require explicit approval when it performs an outward-facing or hard-to-reverse action. For documentation, deployment is complete when the reviewed files are merged and visible on GitHub.

#### Scenario: Documentation change is deployed

- **WHEN** an approved documentation pull request is merged and its files are visible in the target GitHub repository
- **THEN** the lifecycle records deployment as complete

#### Scenario: Deployment awaits approval

- **WHEN** publication, merge, release, or installation requires user approval
- **THEN** the lifecycle pauses before the action and does not infer approval from an earlier phase

### Requirement: Lifecycle work resumes from durable state

The lifecycle MUST derive progress from validated artifacts, recorded approvals, version-control state, pull-request state, and deployment evidence. Resuming MUST continue from the earliest incomplete or invalid phase rather than recreating accepted work.

#### Scenario: Lifecycle resumes after interruption

- **WHEN** an interrupted session restarts with accepted specification and decision artifacts present
- **THEN** the lifecycle verifies those artifacts and continues from the next incomplete phase

#### Scenario: Accepted input changed

- **WHEN** a previously accepted artifact has changed since approval
- **THEN** the lifecycle invalidates dependent progress and returns to review of the changed phase
