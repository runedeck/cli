---
type: proof
change: launch-secret-declaration-lookup
head: 206e206aee13976c3d523119cfad82d825a33f05
head_change: ztxqkpyrxlpvlvtrsvxnqwoxwzqnvyxv
head_tree: 606864b7866cbfef1cd97ad0310bb5fdb5930774
recorded: 2026-11-05
transcript: 4fd4bc6b5d17ea39667cd5ec03889d86030a0cfb00a231ebbcca47efbba064f1
scenes:
- scenario: launch-secret-declaration-lookup#credentials-read-through-the-secret-manager/credential-absent-from-the-environment
  kind: check
- scenario: launch-secret-declaration-lookup#process-environment-first/credential-set-in-the-environment
  kind: check
- scenario: launch-secret-declaration-lookup#named-declaration/named-declaration-missing
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
