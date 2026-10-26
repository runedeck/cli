---
type: proof
change: nothing-installs-without-approval
head: 8919cb32b0673cfab6e8c3f052fbc3000266a7de
head_change: uytrtrrrwuvsvmrmzvwzmtwmwqqmrrpr
head_tree: b7f4ede29878a7db517e43ab52e13861cd40e8df
recorded: 2026-10-26
transcript: b956d027e53b68fdfbc4844c70985a55abaf34570198e8cda28fa4cf531b0a5c
scenes:
- scenario: approve-plan-before-execution#setup-plan-review/user-approves-the-printed-plan
  kind: check
- scenario: approve-plan-before-execution#plan-only-mode/plan-mode-runs-in-automation
  kind: check
- scenario: approve-plan-before-execution#verified-completion-record/verification-check-fails
  kind: check
- scenario: approve-plan-before-execution#first-run-nudge/bare-invocation-finds-no-user-config
  kind: check
- scenario: approve-plan-before-execution#agent-guide/guide-orders-its-steps
  kind: check
- scenario: config-validation-provides-hints#config-check-reporting/unknown-key-appears-in-source-config
  kind: check
- scenario: config-validation-provides-hints#config-check-is-read-only/check-runs-on-a-broken-config
  kind: check
- scenario: config-validation-provides-hints#commented-defaults/defaults-print-for-the-user-scope
  kind: check
- scenario: config-validation-provides-hints#scoped-key-reset/reset-removes-an-unknown-key
  kind: check
- scenario: config-validation-provides-hints#config-reference-output/reference-covers-a-new-key
  kind: check
- scenario: config-validation-provides-hints#reference-drift-check/committed-reference-drifts
  kind: unproven
- scenario: every-error-names-its-fix#stable-error-codes/unknown-config-key-fails-twice
  kind: unproven
- scenario: every-error-names-its-fix#fix-commands-in-errors/error-names-its-repair
  kind: check
- scenario: one-registry-for-harnesses#provider-lifecycle-states/managed-file-is-missing
  kind: check
- scenario: one-registry-for-harnesses#modified-state-protection/repair-meets-a-modified-file
  kind: check
- scenario: one-registry-for-harnesses#provider-explanation/explain-reports-one-provider
  kind: check
- scenario: one-registry-for-harnesses#bounded-detection-evidence/detection-runs-on-a-clean-machine
  kind: check
- scenario: one-registry-for-harnesses#shared-detection-registry/doctor-discovers-providers
  kind: check
- scenario: one-registry-for-harnesses#syntax-preserving-provider-edits/toggle-touches-one-key
  kind: unproven
---

# Proof: nothing-installs-without-approval

This proof covers #50, the setup wizard, together with #43, #44, #47, and #48. Each scene builds its own fixture in its scene directory: a scratch `HOME`, a `PATH` cut down to the `rune` binary and the system directories so that the harnesses of the recording machine do not count, and a `SHELL` that sends shell completion under the scratch `HOME`. The deployment scenes copy the `writing` module from `tests/support/deck` and install it for codex. The approval scene answers the wizard's prompts on standard input. The verification scene points the user config at the completion file through a symlink, so the completion write breaks the config after apply. Three scenes stay unproven: `rune provider enable` drops the comments of `config.yaml`, the reference drift check is a `cargo test` case that needs a rebuilt binary, and the stable-code scenario compares two releases.

## approve-plan-before-execution#setup-plan-review/user-approves-the-printed-plan

```console
$ sh -s
< bin=$(mktemp -d) && ln -s "$(command -v rune)" "$bin/rune"
< export HOME="$PWD/home" SHELL=/bin/bash PATH="$bin:/usr/bin:/bin"
< mkdir -p home/.agents home/Developer/demo && echo "name: demo" > home/Developer/demo/deck.yaml
< printf 'y\ny\ny\ny\ny\n' | rune setup
< find . -type f | sort
use deck [..]/home/Developer/demo? [Y/n] use the detected provider set (agentskills)? [Y/n] install bash shell completions? [Y/n] install the Rune agent skill? [Y/n]  Setup plan
   write        [..]/home/.config/rune/config.yaml (set deck)
   write        [..]/config.yaml (set provider selection)
   write        [..]/home/.local/share/bash-completion/completions/rune (install bash completion)
   write        [..]/home/.agents/skills/rune/SKILL.md (install agent skill)
   write        [..]/home/.config/rune/config.yaml (write verified setup record)

 Provider selection
   agentskills  enabled
   claude       disabled
   codex        disabled
   gemini       disabled
   opencode     disabled

 Notes
   ⚡ deck selected: [..]/home/Developer/demo
   ⚡ providers detected: agentskills
   ⚡ no target bound. Use rune target <slug-or-path> after setup.
Apply this plan? [y/N][..]
 Apply
   ✓ wrote deck: [..]/home/.config/rune/config.yaml
   ✓ wrote provider selection: [..]/config.yaml
   ✓ agentskills enabled
   ✓ claude disabled
   ✓ codex disabled
   ✓ gemini disabled
   ✓ opencode disabled
   ✓ wrote shell completion: [..]/home/.local/share/bash-completion/completions/rune
   ✓ wrote agent skill: [..]/home/.agents/skills/rune/SKILL.md

 Verification
   ✓ user config parses: [..]/home/.config/rune/config.yaml
...
   ✓ shell completion is current: [..]/home/.local/share/bash-completion/completions/rune
   ✓ agent skill is current: 1 current
   ✓ wrote setup record: [..]/home/.config/rune/config.yaml

 Next
   stage        rune add <id> && rune install
./config.yaml
./home/.agents/skills/rune/SKILL.md
./home/.config/rune/config.yaml
./home/.local/share/bash-completion/completions/rune
./home/Developer/demo/deck.yaml
```

## approve-plan-before-execution#plan-only-mode/plan-mode-runs-in-automation

```console
$ sh -s
< bin=$(mktemp -d) && ln -s "$(command -v rune)" "$bin/rune"
< export HOME="$PWD/home" SHELL=/bin/bash PATH="$bin:/usr/bin:/bin"
< mkdir -p home/.agents home/Developer/demo && echo "name: demo" > home/Developer/demo/deck.yaml
< rune setup --plan --json </dev/null
< find . | sort
{"version":1,"completed_steps":["deck","providers","shell_completion","agent_skill"],"writes":[{"step":"set deck","path":"[..]/home/.config/rune/config.yaml"},{"step":"set provider selection","path":"[..]/config.yaml"},{"step":"install bash completion","path":"[..]/home/.local/share/bash-completion/completions/rune"},{"step":"install agent skill","path":"[..]/home/.agents/skills/rune/SKILL.md"},{"step":"write verified setup record","path":"[..]/home/.config/rune/config.yaml"}],"removals":[],"provider_toggles":[{"provider":"agentskills","enabled":true},{"provider":"claude","enabled":false},{"provider":"codex","enabled":false},{"provider":"gemini","enabled":false},{"provider":"opencode","enabled":false}],"notes":["deck selected: [..]/home/Developer/demo","providers detected: agentskills","no target bound. Use rune target <slug-or-path> after setup."]}
.
./home
./home/.agents
./home/Developer
./home/Developer/demo
./home/Developer/demo/deck.yaml
```

## approve-plan-before-execution#verified-completion-record/verification-check-fails

```console
$ sh -s
< bin=$(mktemp -d) && ln -s "$(command -v rune)" "$bin/rune"
< export HOME="$PWD/home" SHELL=/bin/fish PATH="$bin:/usr/bin:/bin"
< mkdir -p home/.config/rune home/.config/fish/completions
< echo "deck: /tmp/deck" > home/.config/fish/completions/rune.fish
< ln -s ../fish/completions/rune.fish home/.config/rune/config.yaml
< rune setup --yes </dev/null
< echo "exit $?"
< grep -q '^setup:' home/.config/rune/config.yaml || echo "no setup record"
 Setup plan
   write        [..]/config.yaml (set provider selection)
   write        [..]/home/.config/fish/completions/rune.fish (install fish completion)
   write        [..]/home/.config/rune/config.yaml (write verified setup record)
...
 Apply
   ✓ wrote provider selection: [..]/config.yaml
...
   ✓ wrote shell completion: [..]/home/.config/fish/completions/rune.fish

 Verification
   ✗ user config parses: [..]/home/.config/rune/config.yaml is malformed: invalid type: string "function __fish_rune_global_optspecs[..]", expected struct Config at line 2 column 1
...
fatal: Setup verification failed. Rune did not write the setup record.
fix: rune setup --yes
exit 2
no setup record
```

## approve-plan-before-execution#first-run-nudge/bare-invocation-finds-no-user-config

```console
$ sh -s
< export HOME="$PWD/home" && mkdir home && unset RUNE_DECK
< rune </dev/null
< echo "exit $?"
< find . | sort
  ᚱᚢᚾᛖ rune · your runes, deployed
...
  rune <command> --help for flags and details
   next         rune setup
exit 0
.
./home
```

## approve-plan-before-execution#agent-guide/guide-orders-its-steps

```console
$ sh -s
< export HOME="$PWD/home" PATH="$(dirname "$(command -v rune)"):/usr/bin:/bin"
< mkdir home work && cd work
< guide="$RUNE_PROOF_ROOT/docs/agent-guide.md"
< sed -n '/^## Read-only first steps/,/^## Setup path/p' "$guide" | grep '^rune ' | while read -r step; do sh -c "$step" </dev/null >/dev/null 2>&1; echo "ran: $step (exit $?)"; done
< grep -n -m 1 'approval' "$guide"
< cd .. && find . | sort
ran: rune --version (exit 0)
ran: rune context --json (exit 0)
ran: rune status (exit 0)
ran: rune provider (exit 0)
ran: rune doctor --target . (exit 2)
45:Ask the human for approval before every write step.
.
./home
./work
```

## config-validation-provides-hints#config-check-reporting/unknown-key-appears-in-source-config

```console
$ sh -c 'printf "providrs:\n    claude:\n        enabled: false\n" > config.yaml'
$ rune config check --scope source
? 1
   ⚡ [..]/config.yaml [config.unknown_key]
   key          providrs
   impact       Rune ignores this key, so it has no effect.
   fix          rune config reference --json
```

## config-validation-provides-hints#config-check-is-read-only/check-runs-on-a-broken-config

```console
$ sh -s
< export HOME="$PWD/home" && mkdir -p home/.config/rune
< printf 'deck: [unclosed\n' > home/.config/rune/config.yaml
< printf 'providers: {codex: \n' > config.yaml
< before=$(cksum home/.config/rune/config.yaml config.yaml)
< rune config check
< echo "exit $?"
< [ "$(cksum home/.config/rune/config.yaml config.yaml)" = "$before" ] && echo "both files byte-identical"
< find . -type f | sort
   ✗ [..]/home/.config/rune/config.yaml [config.invalid]
   impact       Rune cannot load this configuration file.
   fix          rune config defaults --scope user
   ✗ [..]/config.yaml [config.invalid]
   impact       Rune cannot load this configuration file.
   fix          rune config defaults --scope source
exit 1
both files byte-identical
./config.yaml
./home/.config/rune/config.yaml
```

## config-validation-provides-hints#commented-defaults/defaults-print-for-the-user-scope

```console
$ sh -s
< export HOME="$PWD/home" && mkdir -p home/.config/rune
< rune config defaults --scope user > home/.config/rune/config.yaml
< head -n 5 home/.config/rune/config.yaml
< rune config check --scope user
# This file shows the Rune user configuration defaults.
# Copy only the values that you must override.
# Paths can start with a tilde.
deck: null
env: ~/.env
   ✓ Configuration is clean.
```

## config-validation-provides-hints#scoped-key-reset/reset-removes-an-unknown-key

```console
$ sh -c 'printf "# local overlay\nprovidrs:\n    claude:\n        enabled: false\nvalidate:\n    exclude: [build]\n" > config.yaml'
$ rune config reset providrs --scope source
   ✓ removed providrs
   backup       [..]/config.yaml.rune-backup-[..]Z
   restore      command cp [..]/config.yaml.rune-backup-[..]Z [..]/config.yaml
$ sh -c 'diff config.yaml.rune-backup-* config.yaml'
? 1
2,4d1
< providrs:
<     claude:
<         enabled: false
$ cat config.yaml
# local overlay
validate:
    exclude: [build]
```

## config-validation-provides-hints#config-reference-output/reference-covers-a-new-key

```console
$ sh -c 'HOME="$PWD" rune config reference --json | grep -B 1 -A 1 "\"key\": \"setup"'
      "default": null,
      "key": "setup",
      "type": "object | null"
--
      "default": null,
      "key": "setup.completed",
      "type": "array<string>"
--
      "default": null,
      "key": "setup.version",
      "type": "integer"
```

## config-validation-provides-hints#reference-drift-check/committed-reference-drifts

Unproven: the drift check is the `cargo test` case `tests/config_reference.rs`, and a drift needs a binary built from an edited config struct, which the recorder cannot build.

```console
```

## every-error-names-its-fix#stable-error-codes/unknown-config-key-fails-twice

Unproven: the scenario compares the errors of two rune releases, and the recorder runs one binary.

```console
```

## every-error-names-its-fix#fix-commands-in-errors/error-names-its-repair

```console
$ sh -s
< export HOME="$PWD/home" && mkdir home
< printf 'providers: [codex]\n' > config.yaml
< rune provider enable codex
< echo "exit $?"
< rune provider enable codex --json 2>/dev/null
< human=$(rune provider enable codex 2>&1 | sed -n 's/^fix: //p')
< fix=$(rune provider enable codex --json 2>/dev/null | sed -n 's/.*"fix_command":"\([^"]*\)".*/\1/p')
< [ "$human" = "$fix" ] && echo "human and JSON fix commands match"
< echo "running: $fix" && sh -c "$fix"
< echo "exit $?"
warning: config key `providers` has incompatible types (defaults: mapping, override: sequence); keeping default
fatal: config.yaml providers: is not a map
fix: cd [..] && rune config check --scope source
exit 2
{"code":"provider.config_invalid","fix_command":"cd [..] && rune config check --scope source","message":"config.yaml providers: is not a map"}
human and JSON fix commands match
running: cd [..] && rune config check --scope source
   ✗ [..]/config.yaml [config.invalid]
   impact       Rune cannot load this configuration file.
   fix          rune config defaults --scope source
exit 1
```

## one-registry-for-harnesses#provider-lifecycle-states/managed-file-is-missing

```console
$ sh -s
< cp -Rf "$RUNE_PROOF_ROOT/tests/support/deck/runes/writing" src && cd src
< export HOME="$PWD" PATH="$(dirname "$(command -v rune)"):/usr/bin:/bin"
< rune install --provider codex >/dev/null
< rune provider status codex
< rm -f .agents/skills/OnlyWriting/SKILL.md
< rune provider status codex
< rune repair --target .agents
< rune provider status codex
 providers
   codex        current          [..]/src/.codex
 providers
   codex        needs repair     [..]/src/.agents  fix: rune repair --target [..]/src/.agents
 repaired 0 source finding(s), 1 deployment finding(s)
   ✓ restored skills/OnlyWriting/SKILL.md → .agents/skills/OnlyWriting/SKILL.md
 providers
   codex        current          [..]/src/.codex
```

## one-registry-for-harnesses#modified-state-protection/repair-meets-a-modified-file

```console
$ sh -s
< cp -Rf "$RUNE_PROOF_ROOT/tests/support/deck/runes/writing" src && cd src
< export HOME="$PWD" PATH="$(dirname "$(command -v rune)"):/usr/bin:/bin"
< rune install --provider codex >/dev/null
< echo "my own notes" >> .agents/skills/OnlyWriting/SKILL.md
< before=$(cksum < .agents/skills/OnlyWriting/SKILL.md)
< rune repair --json
< [ "$(cksum < .agents/skills/OnlyWriting/SKILL.md)" = "$before" ] && echo "SKILL.md unchanged"
< tail -n 1 .agents/skills/OnlyWriting/SKILL.md
< rune provider status codex
{
  "source_repairs": [],
  "source_faults": [],
  "deployment": [],
  "remaining": [
    {
      "provider": "codex",
      "target": "./.agents",
      "findings": [
        {
          "path": "skills/OnlyWriting/SKILL.md",
          "status": "modified"
        }
      ]
    },
    {
      "provider": "codex",
      "target": "./.codex",
      "findings": []
    }
  ]
}
SKILL.md unchanged
my own notes
 providers
   codex        modified         [..]/src/.agents  review: rune doctor --target [..]/src/.agents
```

## one-registry-for-harnesses#provider-explanation/explain-reports-one-provider

```console
$ sh -s
< cp -Rf "$RUNE_PROOF_ROOT/tests/support/deck/runes/writing" src && cd src
< mkdir bin && printf '#!/bin/sh\n' > bin/codex && chmod +x bin/codex
< export HOME="$PWD" PATH="$PWD/bin:$(dirname "$(command -v rune)"):/usr/bin:/bin"
< rune install --provider codex >/dev/null
< rune provider explain codex --json
{
  "provider": "codex",
  "config_source": "bundled",
  "target": "[..]/src/.codex",
  "evidence": [
    {
      "kind": "executable",
      "value": "codex",
      "result": "found"
    },
    {
      "kind": "config_directory",
      "value": "[..]/src/.codex",
      "result": "found"
    },
    {
      "kind": "deployment_manifest",
      "value": "[..]/src/.agents/.manifest",
      "result": "found"
    },
    {
      "kind": "managed_file",
      "value": "[..]/src/.agents/skills/OnlyWriting/SKILL.md",
      "result": "current"
    },
    {
      "kind": "deployment_manifest",
      "value": "[..]/src/.codex/.manifest",
      "result": "found"
    },
    {
      "kind": "managed_file",
      "value": "[..]/src/.codex/AGENTS.md",
      "result": "current"
    }
  ],
  "deployment_state": "current",
  "fix_command": null,
  "recommended_action": "none"
}
```

## one-registry-for-harnesses#bounded-detection-evidence/detection-runs-on-a-clean-machine

```console
$ sh -s
< mkdir bin && printf '#!/bin/sh\n/usr/bin/touch "%s/codex-ran"\n' "$PWD" > bin/codex && chmod +x bin/codex
< printf 'version: 1\nproviders:\n    codex:\n        executable: sh\n' > provider-detection.yaml
< export HOME="$PWD" PATH="$PWD/bin:$(dirname "$(command -v rune)"):/usr/bin:/bin"
< rune provider explain codex
< [ -e codex-ran ] || echo "codex never ran"
 codex
   config       bundled
   target       [..]/.codex
   state        not installed

 evidence
   executable           codex  found
   config directory     [..]/.codex  not found
   deployment manifest  [..]/.agents/.manifest  not found
   deployment manifest  [..]/.codex/.manifest  not found

review: rune context
codex never ran
```

## one-registry-for-harnesses#shared-detection-registry/doctor-discovers-providers

```console
$ sh -s
< export HOME="$PWD" PATH="$(dirname "$(command -v rune)"):/usr/bin:/bin"
< mkdir .unknown && printf 'skills: {}\n' > .unknown/.manifest
< rune doctor --target .unknown
< rune provider status
fatal: cannot infer provider for .unknown; expected one of .agents, .claude, .agents, .codex, .gemini, .opencode
fix: rune provider status
 providers
   agentskills  disabled         [..]/.agents  fix: rune provider enable agentskills
   claude       not installed    [..]/.claude  review: rune context
   codex        not installed    [..]/.codex  review: rune context
   gemini       not installed    [..]/.gemini  review: rune context
   opencode     not installed    [..]/.opencode  review: rune context
```

## one-registry-for-harnesses#syntax-preserving-provider-edits/toggle-touches-one-key

Unproven: `rune provider enable` renders `config.yaml` again through `serde_yaml`, which drops every comment, so this scene fails until the syntax-preserving editor of task 3.1 exists.

```console
$ sh -s
< printf '# local overlay\nproviders:\n    # codex stays off for now\n    codex:\n        enabled: false\n' > config.yaml
< HOME="$PWD" rune provider enable codex >/dev/null
< cat config.yaml
# local overlay
providers:
    # codex stays off for now
    codex:
        enabled: true
```
