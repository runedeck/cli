---
title: "Plugin Deploy Shape"
description: "The claude provider deploys skills, agents, and hooks as a skills-directory plugin so the harness namespaces them as rune:<skill>"
type: adr
category: cli
tags:
    - cli
    - deploy
    - claude
    - namespace
status: accepted
created: 2026-07-18
updated: 2026-07-18
author: "@N4M3Z"
project: rune-cli
related:
    - "CLI-0019 Singular Noun Subcommands"
    - "ASSEMBLY-0003 Manifest-Based Deployment Tracking"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Plugin Deploy Shape

## Context and Problem Statement

Skills deployed loose into `.claude/skills/<name>` load unnamespaced: `/deslop` can collide with bundled or project skills, and nothing brands the deck's content. Claude Code's plugin system namespaces skills as `<plugin>:<skill>`, but the classic route to a plugin is a marketplace install — heavier than `rune skill add … && rune install`, the primary path. The docs also define [skills-directory plugins](https://code.claude.com/docs/en/plugins-reference#skills-directory-plugins): any folder under a skills directory carrying `.claude-plugin/plugin.json` auto-loads as `<name>@skills-dir` with no marketplace and no install step.

## Decision Drivers

- The `rune:` namespace must arrive through the primary staging path, not only through marketplace packaging
- Loose skills cannot carry a prefix; only plugin packaging namespaces
- Hooks should register through the plugin loader instead of hand-wired settings.json entries
- Manifest-based prune, doctor, and drift must keep managing every deployed byte

## Considered Options

1. **Loose layout with prefixed names** (`.claude/skills/rune-deslop`): reads as a prefix but is not the `rune:` namespace, and pollutes bare skill names.
2. **Marketplace plugin only**: real namespacing, but decouples deployment from `rune install` and adds an install ceremony per consumer.
3. **Deploy the selection as a skills-directory plugin**: `rune install` writes `.claude/skills/rune/` as a plugin root; the harness auto-loads it per session.

## Decision Outcome

Chosen option: **Option 3.**

- A provider config key `plugin: <name>` (default `plugin: rune` for claude) rewrites the target map: skills, agents, and hooks deploy under the plugin root `<target>/skills/<plugin>/`, rules keep their loose path (rules are not a plugin component). An explicit by-kind target map wins over the derivation.
- Deploy generates two manifest-tracked files at the plugin root: `.claude-plugin/plugin.json` (the namespace source) and `hooks/hooks.json`, the union of every deployed domain hook table in sorted domain order. Generated files respect user modifications like any deployed file.
- Assembly keeps `${CLAUDE_PLUGIN_ROOT}` in hook commands (the loader defines it) and adds the domain segment: `${CLAUDE_PLUGIN_ROOT}/hooks/<domain>/<script>`.
- Doctor scans nested managed roots (`skills/<plugin>/.manifest`) as their own targets and excludes them from the outer root's orphan scan; drift treats the generated files as expected.
- `plugin: null` in a consumer's config restores the loose layout.

Verified live: `claude plugin validate` passes on the generated root, and a Claude Code session with the deployed plugin lists every deck skill as `rune:<skill>`.

## Consequences

- Skills gain the `rune:` namespace on the primary path; hooks register without settings.json wiring.
- The `.claude` target now carries two manifests (loose rules at `.claude/.manifest`, plugin tree at `.claude/skills/rune/.manifest`); an upgrade prunes the old loose files through the existing manifest, quarantined to `.trash/`.
- Project-scope plugins load only after workspace trust and only from the session's starting directory; personal-scope (`~/.claude/skills/rune`) loads everywhere.
- Skill body edits are live in a session; hook and agent changes need `/reload-plugins`.
