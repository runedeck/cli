# Skill layers

## Purpose

Check declared source layers and rendered instructions against their declared policies without claiming
model-quality evidence. Identity and readiness are the sibling capabilities of this change.

## ADDED Requirements

### Requirement: CSI-08 Source layers and rendered instructions satisfy their declared policies

`rune validate --skill-layers --source <skill-folder|module|deck>` MUST provide a strict read-only source gate.
It MUST preserve ordinary validation and assembly compatibility outside this opt-in mode.
It MUST check every authored skill layer and text companion without silently skipping malformed or unsupported input.
Generic and user content MUST permit portable Agent Skills metadata and existing source routing and invocation controls.
Those portable layers MUST reject enumerated harness-only symbols and other harness runtime frontmatter.
Harness variants MUST permit their own declared metadata and symbols and MUST reject foreign harness dependencies.
Each model variant MUST use an exact configured model ID beneath its own provider and MUST contain only `mode` metadata.
Every variant MUST declare `mode: append`, `mode: prepend`, or `mode: replace` explicitly.
The complete `user/SKILL.md` replacement MUST declare `mode: replace`, complete identity, and the canonical skill name.
It MUST replace the whole entrypoint without merging canonical base metadata or body.
Model checks MUST preserve skill identity and routing and MUST inherit their harness's policy.
They MUST NOT be reported as evidence of model behavior, tool availability, or quality.

The gate MUST apply combined policies without changing the existing single-winner variant merge.
It MUST derive effective provider applicability from the selected base or complete user entrypoint.
Content targets MUST use resolved provider names, aliases, and target directories through the existing content matcher.
One content selector MUST preserve every matching provider independently of unique CLI provider-argument resolution.
Malformed provider configuration MUST fail without a fallback that discards authored settings.
Metadata-only append/prepend variants MAY have empty bodies when the resolved entrypoint remains complete and nonempty.
The source gate MUST check merged source and applicable shared companions before provider transformation and filtering.
Rendered inspection MUST check actual entrypoints and emitted text companions after those assembly steps.
Doctor MUST check rendered Codex bundles with the shared Codex policy and report `CSI007_HARNESS_LEAKAGE` violations.
The literal-symbol rules MUST include quoted examples and conditional prose without arbitrary exemptions.
They MUST NOT broadly prohibit provider names or `.codex` path mentions.
Unknown or misplaced model qualifiers, missing explicit modes, unreadable required text, and zero checked skills MUST fail.
An unavailable source MUST return the versioned report with `SL000_INVALID_INPUT` and its requested path.
The gate MUST report findings without automatic rewrites or suppressions.

#### Scenario: Portable base and valid harness variant

- **WHEN** a portable skill has a supported harness variant with explicit mode and permitted harness metadata
- **THEN** the source gate checks both layers and their resolved output
- **AND** it preserves shared companion references and the existing single-winner merge behavior

#### Scenario: Model qualifier violates the declared contract

- **WHEN** a model variant uses an unknown ID, sits under another provider, or changes identity or routing metadata
- **THEN** the gate rejects the variant and identifies its source path
- **AND** it does not silently fall back to a passing provider or base check

#### Scenario: User override is a complete replacement

- **WHEN** `user/SKILL.md` supplies a complete portable entrypoint with `mode: replace` and the canonical skill name
- **THEN** source inspection uses that complete entrypoint and its provider applicability
- **AND** omitted canonical metadata does not reappear through a simulated base merge

#### Scenario: Appended content or companion retains a foreign dependency

- **WHEN** a merged entrypoint or emitted companion contains an enumerated foreign harness symbol
- **THEN** the rendered check reports the affected path and literal symbol
- **AND** clean variant text cannot conceal the retained dependency

#### Scenario: Quoted symbol and ordinary provider reference

- **WHEN** quoted text contains an enumerated foreign harness symbol
- **THEN** the literal policy reports it without an example exemption
- **AND** an ordinary provider name or `.codex` path mention alone does not produce that finding

#### Scenario: Source checks pass without model evidence

- **WHEN** all source-layer and rendered checks pass without a native session or model evaluation
- **THEN** the record reports declared static constraints only
- **AND** the native acceptance requirements remain unchanged

#### Scenario: Configured content selector matches several providers

- **WHEN** a source target matches several configured provider aliases or target directories
- **THEN** source inspection checks every matching provider
- **AND** it does not apply the unique CLI provider-argument resolver to that content selector

#### Scenario: Requested source is unavailable

- **WHEN** the source path cannot be resolved
- **THEN** the command returns the versioned layer report with the requested path and `SL000_INVALID_INPUT`
- **AND** it reports zero checked paths and `valid: false`
