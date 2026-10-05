---
title: "Deployed skill names are a provider rule"
description: "A skill keeps its PascalCase source identity in the deck, and a provider whose specification asks for kebab-case names converts the skill tree and its links at assembly."
type: adr
category: assembly
tags:
    - assembly
    - providers
    - skills
status: proposed
created: 2026-08-06
updated: 2026-10-06
author: "@N4M3Z"
project: rune-cli
related:
    - "ASSEMBLY-0007 Cross-Provider Convergence"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream:
    - "kebab-case-skill-deployment#skill-paths-deploy-in-kebab-case"
    - "https://github.com/runedeck/cli/pull/11"
change: kebab-case-skill-deployment
---

# Deployed skill names are a provider rule

## Context and Problem Statement

A deck names a skill once, in its directory, its frontmatter `name`, and its H1, and the stable shell convention requires the three to agree. The deck writes those names in PascalCase, such as `BuildSkill`. The Agent Skills specification requires a `name` of lowercase letters, digits, and single hyphens that matches the parent directory ([Agent Skills specification][AGENTSKILLS]). The other providers in `defaults.yaml` set no casing rule for skills. A skill tree also holds files that must keep their names: `SKILL.md`, which every harness finds by its spelling, scripts whose names a language must import, and assets that other files name. The question is where the deployed name comes from: the source, or the provider that deploys it.

## Considered Options

1. Rename every skill at the source to kebab-case, and keep one casing from deck to harness.
2. Reject PascalCase in validation, so a deck cannot author a name that a provider would refuse.
3. Keep the source identity as the deck writes it, and let a provider assembly rule convert the deployed tree: directories, Markdown companions, the frontmatter `name`, and the links between them.

## Decision Outcome

Option 3.

- A provider whose specification asks for kebab-case MUST enable the `kebab-case-skills` assembly rule. In `defaults.yaml` that provider is `agentskills`.
- The rule MUST convert directory segments and Markdown companion names, and MUST keep `SKILL.md`, scripts, and assets under their source file names.
- The rule MUST retarget the relative links in each converted Markdown file, so a deployed document never names a file that is not there.
- The rule MUST be idempotent, so a skill authored in kebab-case passes through unchanged.
- Validation MUST accept any source casing that the rule can normalize and MUST reject a name with a leading, trailing, or repeated separator.

Option 1 forces one provider's naming rule on every other provider and on the deck's own conventions, and it gives up names that read as identifiers in prose. Option 2 is option 1 enforced by the validator. Both put a destination property into the source.

### Consequences

- [+] A deck keeps one source identity for a skill, and each provider decides the deployed name.
- [+] A provider without the rule deploys the skill under the names the deck wrote.
- [+] A link in `SKILL.md` to a companion resolves after deployment, because one function converts both the file path and the link target.
- [-] The deployed path differs from the source path on `agentskills`, so a reference from outside the skill tree to the source directory name does not resolve there.
- [-] The frontmatter of each converted Markdown file is written out again by the YAML serializer, so quoting and key layout can change even when the name is already kebab-case.
- [-] Link retargeting covers inline links and reference definitions only. A path in a code span or in plain text keeps its source spelling.
- [-] ASSEMBLY-0007 lists name conversion as transitional, so the rule is code that rune keeps until the providers agree on one casing.

[AGENTSKILLS]: https://agentskills.io/specification
