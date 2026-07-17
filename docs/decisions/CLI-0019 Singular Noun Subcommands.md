---
title: "Singular Noun Subcommands"
description: "Noun namespaces are singular with plural aliases, the consumer manifest stays .rune, and both staging surfaces are canonical"
type: adr
category: cli
tags:
    - cli
    - naming
    - ux
status: accepted
created: 2026-07-17
updated: 2026-07-17
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0015 Git-Style External Command Dispatch"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Singular Noun Subcommands

## Context and Problem Statement

rune grew kind-scoped staging commands (`rune skill add`, `rune agent add`, `rune rule add`, `rune hook add`) alongside the generic `rune add <id>`. Three naming questions need one ruling: singular or plural noun namespaces, `.rune` or `.runes` for the consumer manifest, and which staging surface the documentation teaches.

## Decision Drivers

- Match the conventions users already know from git, gh, docker, and terraform
- Keep exactly one taught spelling while forgiving the instinctive alternative
- Avoid a manifest rename and migration without a principled reason
- Keep casts and whole-domain staging expressible after the kind commands exist

## Considered Options

1. **Plural canonical** (`rune skills add`), Heroku-style: the bare topic doubles as the list command, but it departs from the git/gh/docker mental model closest to rune's shape.
2. **Singular canonical, no aliases**: cleanest surface, but `rune skills` and `rune completions` fail for no user benefit.
3. **Singular canonical with hidden plural aliases**, kubectl-style tolerance, bare noun lists the collection.

## Decision Outcome

Chosen option: **Option 3.**

- **Noun namespaces are singular**: `rune skill`, `rune agent`, `rune rule`, `rune hook`, `rune target`, `rune spec`, matching git (`git remote add`, `git stash`), gh (`gh repo`, `gh pr`, and `gh skill` in this exact domain), docker management commands (`docker container create`), terraform (`terraform workspace`), npm noun groups (`npm config`, `npm team`), and crex (`crex template`).
- **Plural forms are hidden aliases** (`rune skills add`, `rune completions`), following kubectl, which accepts singular, plural, and short names — every CRD defines `names.singular` as "an alias on the CLI" ([CRD docs][K8SCRD]), and a core maintainer called REST pluralization "mostly human conceit" ([issue 18622][K8S18622]). Help, docs, and completions teach only the singular form.
- **The bare noun lists the collection**: `rune skill` with no subcommand lists the source deck's skills with staged markers, capturing the one real advantage of Heroku's plural mandate ("topics are plural nouns", bare topic lists, [style guide][HEROKU]) inside the singular scheme.
- **The consumer manifest stays `.rune`**: tool-named dotfiles use the bare tool name (`.git`, `.cargo`, `.npmrc`, `.vscode`); the plural precedents (`.gitmodules`, `.gitattributes`) are content-named manifests.
- **Both staging surfaces are canonical**: `rune add` for casts, whole domains, and qualified ids; the kind commands for staging by bare name. Docker's parallel top-level and management-command surfaces are the precedent.

The Command Line Interface Guidelines ([clig.dev][CLIG]) rule on noun-verb ordering and consistency but take no position on noun number; Microsoft's System.CommandLine guidance asks only that an app "be consistent in pluralization" ([.NET design guidance][DOTNET]).

## Consequences

- One taught spelling; the instinctive plural never errors, at the cost of one clap alias attribute per namespace.
- `.rune` needs no migration, and the name keeps working if the file later tracks more than rune selections.
- The bare-noun listing adds a read-only surface to every kind namespace that stays in sync with staging.

[K8SCRD]: https://kubernetes.io/docs/tasks/extend-kubernetes/custom-resources/custom-resource-definitions/
[K8S18622]: https://github.com/kubernetes/kubernetes/issues/18622
[HEROKU]: https://devcenter.heroku.com/articles/cli-style-guide
[CLIG]: https://clig.dev/
[DOTNET]: https://learn.microsoft.com/en-us/dotnet/standard/commandline/design-guidance
