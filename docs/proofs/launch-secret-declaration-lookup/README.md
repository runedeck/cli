---
type: proof
change: launch-secret-declaration-lookup
head: 0d027ed150d912b5dff7bc2eaa7a45a152dca508
head_change: usprswrvwzoswvywlwvoqnporzqnrxvw
head_tree: 2409266ca037be195fb27a0c5a7e45ad16ded06f
recorded: 2026-11-14
transcript: 0604106c055b236a9da7534ee334af1547a28d516a47e3b646ef3bac821237a9
scenes:
- scenario: launch-secret-declaration-lookup#credentials-read-through-the-secret-manager/credential-absent-from-the-environment
  kind: check
- scenario: launch-secret-declaration-lookup#process-environment-first/credential-set-in-the-environment
  kind: check
- scenario: launch-secret-declaration-lookup#named-declaration/named-declaration-missing
  kind: check
- scenario: launch-secret-declaration-lookup#named-secret-profile/configured-secret-profile
  kind: check
- scenario: launch-secret-declaration-lookup#default-declaration/no-declaration-on-the-machine
  kind: check
- scenario: launch-secret-declaration-lookup#default-declaration/named-environment-file-beside-a-default-declaration
  kind: check
- scenario: launch-secret-declaration-lookup#failed-read-stops-the-launch/secret-manager-refuses
  kind: check
- scenario: launch-secret-declaration-lookup#read-reason-recorded/audit-entry-for-a-launch
  kind: check
- scenario: launch-secret-declaration-lookup#one-read-per-credential/two-values-name-one-credential
  kind: check
---

# Proof: launch-secret-declaration-lookup

One scene per scenario, in closure order. Each scene runs the real `rune` binary with `HOME=.` in its scratch directory. The fixture in `tests/fixtures/launch-secret-declaration-lookup/` holds a rune config with the `probe` and `twice` profiles, the default secret declaration, an env file that holds `PROOF_ORIGIN=from-env-file`, and two stubs. `bin/secretspec` stands in for secretspec: it logs each call and its arguments to `secretspec.log`, refuses a key that the declaration does not list, and prints `from-secretspec`, or fails like a dotenv provider that cannot parse its store. `bin/claude` stands in for Claude Code: it writes `claude-started` and prints `PROOF_VALUE`. Its `home.sh` copies them into the scene directory. No scene reads a real secret store, calls a model, or starts a real harness. The dry-run `env_removed` block depends on the parent environment, so the scenes elide it.

The secret profile scene runs the real `secretspec` instead of the stub. Its `profiles.sh` removes `bin/secretspec` and installs `profiles.toml` as the default declaration. That declaration names the profiles `owner` and `agents` and no `default`, and keeps each fake value in a `file:` store beside it. The runner gives each scene a fresh temporary directory, and `HOME=.` keeps secretspec's config and audit log inside it, so the scene never reads a real secret store.

`PROOF_ORIGIN` and `PROOF_VALUE` carry no credential word. A dry run therefore hides a value only when the value came through the secret declaration, and prints a value from the shell or an env file as it is.

## launch-secret-declaration-lookup#credentials-read-through-the-secret-manager/credential-absent-from-the-environment

```console
$# the env file holds PROOF_ORIGIN, and the default declaration sits beside the rune config
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh" && cat .env && ls .config/rune'
PROOF_ORIGIN=from-env-file
config.yaml
secretspec.toml
$# nobody may read the env file now, so a launch that opened it would fail
$ chmod 000 .env
$# the shell does not set PROOF_ORIGIN, so the launcher asks the secret manager, and the env file stays unread
$# the dry run hides the value, because it came through the declaration
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch probe@claude --dry-run'
tool: claude
argv: claude
env:
  PROOF_VALUE=<redacted>
...
$ grep -c '^call$' secretspec.log
1
```

## launch-secret-declaration-lookup#process-environment-first/credential-set-in-the-environment

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh"'
$# the shell sets PROOF_ORIGIN, so the launch uses that value
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. PROOF_ORIGIN=from-parent rune launch probe@claude --dry-run'
tool: claude
argv: claude
env:
  PROOF_VALUE=from-parent
...
$# the secret manager received no request, so it wrote no log
$ ls
bin
```

## launch-secret-declaration-lookup#named-declaration/named-declaration-missing

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh"'
$# the rune config names a declaration that does not exist
$ sh -c 'printf "secrets: missing/secretspec.toml\n" >> .config/rune/config.yaml'
$# the launch stops, and the report names the credential and the missing path
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch probe@claude'
? 2
fatal: profile references PROOF_ORIGIN, unset in the environment, and the secrets manifest missing/secretspec.toml does not exist
$# the harness never started, and the secret manager received no request
$ ls
bin
$# a declaration named in the process environment wins over the config, and the harness starts with the value
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. RUNE_SECRETS=.config/rune/secretspec.toml rune launch probe@claude'
PROOF_VALUE=from-secretspec
```

## launch-secret-declaration-lookup#named-secret-profile/configured-secret-profile

```console
$# the real secretspec answers here: the declaration names the profiles owner and agents and no default, and each profile keeps one fake value in a file store
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/profiles.sh" && ls bin .config/rune/store/proof'
.config/rune/store/proof:
agents
owner

bin:
claude
$# without secrets_profile rune names no profile, secretspec asks for the missing default profile, and the launch stops
$ sh -c 'HOME=. rune launch probe@claude --dry-run'
? 2
fatal: profile references PROOF_ORIGIN, unset in the environment and unreadable from ./.config/rune/secretspec.toml: the manifest declares no such profile; config key secrets_profile names one; `secretspec check --file ./.config/rune/secretspec.toml --explain` prints a trace without values
$# the config names the owner profile, and the config listing shows the value and its source
$ sh -c 'HOME=. rune config set secrets_profile owner'
set secrets_profile = owner
$ sh -c 'HOME=. rune config | grep secrets_profile'
   secrets_profile config   owner
$# the dry run reads from the owner profile and hides the value, and secretspec's audit log names the profile of each read
$ sh -c 'HOME=. rune launch probe@claude --dry-run'
tool: claude
argv: claude
env:
  PROOF_VALUE=<redacted>
...
$ jq -r 'select(.action == "get") | [.profile, .key, .outcome] | join(" ")' .local/state/secretspec/audit.log
default PROOF_ORIGIN error
owner PROOF_ORIGIN found
$# the harness starts with the owner value
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch probe@claude'
PROOF_VALUE=fake-owner-value
$# the setting changes to the agents profile, and the next dry run and launch read from it
$ sh -c 'HOME=. rune config set secrets_profile agents'
set secrets_profile = agents
$ sh -c 'HOME=. rune launch probe@claude --dry-run'
tool: claude
argv: claude
env:
  PROOF_VALUE=<redacted>
...
$ jq -r 'select(.action == "get") | [.profile, .key, .outcome] | join(" ")' .local/state/secretspec/audit.log
default PROOF_ORIGIN error
owner PROOF_ORIGIN found
owner PROOF_ORIGIN found
agents PROOF_ORIGIN found
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch probe@claude'
PROOF_VALUE=fake-agents-value
$# the process environment wins over the config
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. RUNE_SECRETS_PROFILE=owner rune launch probe@claude'
PROOF_VALUE=fake-owner-value
```

## launch-secret-declaration-lookup#default-declaration/no-declaration-on-the-machine

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh"'
$# nothing names a declaration, and the config directory now holds none
$ rm .config/rune/secretspec.toml
$# the launcher reads the unset credential from the env file
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch probe@claude --dry-run'
tool: claude
argv: claude
env:
  PROOF_VALUE=from-env-file
...
$ ls
bin
```

## launch-secret-declaration-lookup#default-declaration/named-environment-file-beside-a-default-declaration

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh"'
$# the rune config names an env file, names no declaration, and the default declaration still exists
$ sh -c 'printf "env: other.env\n" >> .config/rune/config.yaml && printf "PROOF_ORIGIN=from-named-env-file\n" > other.env && ls .config/rune'
config.yaml
secretspec.toml
$# the launcher reads the unset credential from the named env file
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch probe@claude --dry-run'
tool: claude
argv: claude
env:
  PROOF_VALUE=from-named-env-file
...
$ ls
bin
other.env
```

## launch-secret-declaration-lookup#failed-read-stops-the-launch/secret-manager-refuses

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh"'
$# the secret manager fails, and its error output quotes FAKE-LEAK-VALUE, the value of another key
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. PROOF_SECRETSPEC_FAIL=1 rune launch probe@claude'
? 2
fatal: profile references PROOF_ORIGIN, unset in the environment and unreadable from ./.config/rune/secretspec.toml: a provider in the manifest's chain failed to parse its store; `secretspec check --file ./.config/rune/secretspec.toml --explain` prints a trace without values
$# the secret manager ran once, and the harness never started
$ ls
bin
secretspec.log
```

## launch-secret-declaration-lookup#read-reason-recorded/audit-entry-for-a-launch

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh"'
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch probe@claude --dry-run > /dev/null'
$# the request carries a reason that names the profile and the credential, and names rune as the caller
$ cat secretspec.log
call
get
PROOF_ORIGIN
--file
./.config/rune/secretspec.toml
--reason
rune resolves the probe@claude launch profile and reads PROOF_ORIGIN for its environment
--caller
rune
--caller-version
[..]
--caller-operation
resolve-profile
```

## launch-secret-declaration-lookup#one-read-per-credential/two-values-name-one-credential

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/launch-secret-declaration-lookup/home.sh"'
$# PROOF_FIRST and PROOF_SECOND both name PROOF_ORIGIN, and the dry run hides both values
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. rune launch twice@claude --dry-run'
tool: claude
argv: claude
env:
  PROOF_FIRST=<redacted>
  PROOF_SECOND=<redacted>
...
$# the secret manager received one request
$ grep -c '^call$' secretspec.log
1
```
