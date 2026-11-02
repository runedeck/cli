---
adr: docs/changes/rune-installs-profiles-into-interfaces/adr.md
status: proposed
decisions: ["Harness Interfaces"]
---

# Rune installs profiles into interfaces

## Why

The owner runs Claude Code and Codex through one CLIProxyAPI hub and watches sessions in T3 Code and Multica.
Theo's hub guide is the baseline: point each machine's Claude Code and Codex configuration at the hub, and
every interface that runs those CLIs uses it. The guide leaves some steps manual: editing the client files,
the one-hour prompt cache that Claude Code drops on a custom endpoint, the routed model names in T3's model
picker, and checks that the setup still holds. The owner's launch profiles already hold the hub URL, the key
reference, and the model names. This change absorbs Z-88 and replaces Z-122.

## What Changes

- `rune interface enable t3code` applies the guide's client step from the launch profiles: a backup, then the
  hub URL and key reference in Claude Code's settings and a hub model provider in Codex's configuration. It
  writes to a second configuration directory instead when the owner keeps the normal install.
- It sets the one-hour prompt cache for the main conversation on hub-routed Claude Code, and for subagents
  when asked.
- It writes the routed model names into T3's Claude entry, and optionally adds the hub as a T3 usage source.
- `rune interface enable multica` checks that Multica's runtimes inherit the machine configuration. A launch
  wrapper is written only for a profile that needs arguments or middleware an interface cannot express.
- `rune interface status` checks the hub's model list, the cache lifetime, session affinity, Codex websockets,
  Claude subscription models outside Claude Code, and stale ACP entries.
- `rune interface disable <name>` removes only what rune wrote.

## First slice

The 0.7.0 release carries a read-only slice for T3 Code. It never opens the T3 Code settings file for writing.

- `rune interface list` names the known interfaces and whether each is on this machine.
- `rune interface status t3code` reads the settings file and prints its provider instances and the models that a
  launch profile would own.
- `rune interface enable t3code --dry-run` prints the plan: the profiles, the provider instance of each, the keys
  rune would own, and each field that T3 Code cannot represent.
- `rune interface enable t3code` without `--dry-run` refuses, and says that this release writes nothing.

The write, `disable` with the ownership record, the client step, the cache lifetime, Multica, and the validator
checks follow in later changes. The section "T3 findings" of the design says what the slice plans.

## Capabilities

- install-profiles-into-interfaces (new)

## Impact

- New command module under `src/cli/interface/`, routed from `src/cli/mod.rs`. It reads launch profiles
  through the existing configuration loader and reuses the model check behind `rune launch --check`.
- New state record and backups under `~/.local/state/rune/interfaces/`.
- README, CHANGELOG, and the help text.
- Out of scope: installing or running the hub, account logins, routing strategy, and dashboards.
