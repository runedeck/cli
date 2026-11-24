# Design

## Approach

The policy becomes a property of the dispatcher. A handler opts in with one key, `policy: <name>`, and the dispatcher asks the reader about that name before it starts the handler. The reader is a port of the Jev reader in `~/Developer/runedeck/jev/src/policy/`. It keeps the file format, the layer order, the layer labels, the reasons, and the path rules, so `rune policy check` and `jev policy` print the same line for the same question. The Jev test cases are ported one by one into `src/policy/tests.rs`.

The reader adds two layers that the shared context of the Jev integration fixes for both readers: a repository file that may only take away, and a session switch file.

## Runtime

```text
rune hook run (one event, payload cwd and session_id)
    for each planned handler in order
        no policy key          -> as before
        policy key             -> reader.decide(name, cwd or the process cwd, session_id)
            refused            -> no spawn, no failure, log "<id> skipped by policy: <name> (<layer>)",
                                  record skipped = "skipped by policy: <name> (<layer>)", duration 0
            allowed            -> the legacy cover check, then the spawn, as before
    turn.finish: a refused turn wrapper holds no time back from the handlers before it

reader.decide(name, path, session)
    user file missing or broken          -> refused, layer "policy file", reason the fault
    top level grant                      -> "top level" (grant <pattern> | no grant)
    scopes containing path, outermost first
        only   -> allowed iff matched    -> "scope <path>" (only <pattern> | not in only)
        grant  -> allowed                -> "scope <path>" (grant <pattern>)
        revoke -> refused                -> "scope <path>" (revoke <pattern>)
    repository file of path's repository, if any
        broken -> refused                -> "repository <root>" (<fault>)
        only   -> refused if unmatched, unchanged if matched
        revoke -> refused
        grant  -> ignored, one warning
    disabled (user file)                 -> "disabled" (disabled <pattern>)
    disabled (repository file)           -> "repository <root>" (disabled <pattern>)
    session switch file                  -> "session <id>" (disabled <pattern> | <fault>)
    RUNE_POLICY_DISABLE                  -> "RUNE_POLICY_DISABLE" (RUNE_POLICY_DISABLE <pattern>)
```

## Data

- User file: `RUNE_POLICY` when set and not empty, else `$HOME/.config/rune/policy.yaml`. Keys `version` (must be 1), `disabled`, `grant`, `scopes`. A scope has `path`, `only`, `grant`, `revoke`. An absent `only` leaves the answer alone, and `only: []` refuses everything in the scope.
- Repository file: `<root>/.rune/policy.yaml`, where `<root>` is the nearest ancestor of the resolved path that holds `.jj` or `.git`. A `.rune` that is a file, as a consumer manifest is, carries no policy. Keys `version` (must be 1), `disabled`, `grant`, `revoke`, `only`. A grant is ignored with a warning.
- Session switch file: `<state>/policy/session/<session id>.yaml`, where `<state>` is `RUNE_STATE_DIR`, else `$XDG_STATE_HOME/rune`, else `~/.local/state/rune`. Keys `disabled` and an optional `version: 1`. A missing or empty file adds nothing. A broken file refuses every name. A session id with characters other than letters, digits, `.`, `_`, and `-`, or one that starts with `.`, reads no file.
- Patterns: a whole name, or a prefix followed by `.*`, which matches every name below the prefix and not the prefix itself.
- Paths: a leading `~` expands to `HOME`. A relative path resolves against the current directory. The real path comes from the nearest existing ancestor with the rest appended. Letter case folds on macOS and Windows. A scope contains its own path and every path below it.
- Handler key: `policy` in `hooks.handlers[]`, copied into the compiled plan as `PlannedHandler.policy`. It is absent from the plan when the handler names none, so a plan without policies keeps its bytes and its generation name. Validation refuses a name that is empty, holds a character other than lowercase letters, digits, `.`, `_`, and `-`, has an empty segment, or ends in `.*`.
- Hook record: the existing `skipped` field holds `skipped by policy: <name> (<layer>)`. The legacy cover keeps writing the part's path there.
- `rune policy check [--path <dir>] [--session <id>] <name>...`: one line per name, `<name padded>  allowed|refused  <layer> (<reason>)`. Exit 0 when every name is allowed, 1 when any is refused, 2 on a usage error. Warnings go to stderr. The path defaults to the current directory. With `--json` it prints one array of `{name, allowed, layer, reason, warnings}`.
- `rune hook list`: when any handler of the plan names a policy entry, every handler line gains a column after the failure policy with the name, or `-` for none. Without one the output is unchanged.

## Structure

- `src/policy/mod.rs`: `Policy`, `Scope`, `Loaded`, `parse`, `load`, `path_from`, `disabled_from`, `Environment`, `Reader`, `Decision`.
- `src/policy/paths.rs`: `expand_home`, `realpath_nearest`, `normalize`, `contains`, `repository_root`.
- `src/policy/resolve.rs`: `matches`, the layer walk, the repository file, the session switch file.
- `src/policy/tests.rs`: the ported Jev cases and the two new layers.
- `src/hooks.rs`: `Handler.policy`, `PlannedHandler.policy`, the name check in `validate`.
- `src/cli/hook/run.rs`: `Runner.policy`, `refusal`, the skip before the cover check, and no hold for a refused wrapper.
- `src/cli/hook/mod.rs`: the policy column in `render_plan`.
- `src/cli/policy.rs`: `check`, `render`.
- `src/cli/mod.rs`: `Command::Policy { PolicyAction::Check }` and its line in the root help.

## Limits

- A legacy dispatcher part that covers a handler still runs the handler's command. The policy decides only what the dispatcher starts.
- An event without a working directory resolves against the dispatcher's own directory.
- The reader reads the files once per handler that names a policy entry. A plan with many such handlers on one event reads them as many times.
- The layer labels of the two new layers are this design's choice. The Jev reader must print the same labels to agree.

## Coverage

| capability | requirement | check or open gap |
|---|---|---|
| policy-gates-hook-handlers | The Dispatcher Skips A Handler The Policy Refuses | `dispatch_tests.rs`: refused guard does not start and is recorded, allowed guard denies, session switch, missing policy, repository warning once. `tests/hook_policy.rs`: the real binary with a marker file the handler would write. Open gap: no test for the refused wrapper's hold |
| policy-gates-hook-handlers | A Handler Without A Policy Name Runs As Before | `dispatch_tests.rs`: no policy reader at all, a broken policy file never read. `tests/hook_policy.rs`: no policy file |
| policy-gates-hook-handlers | The Policy Reader Decides A Name By Layers | `src/policy/tests.rs`: the ported Jev cases |
| policy-gates-hook-handlers | A Missing Or Broken Policy Refuses Everything | `src/policy/tests.rs`: missing file, unknown key, version, YAML, scope without a path |
| policy-gates-hook-handlers | Paths Compare Through Links And Letter Case | `src/policy/tests.rs`: home expansion, nearest ancestor, link in both directions, case, containment |
| policy-gates-hook-handlers | A Repository Policy Only Takes Away | `src/policy/tests.rs`: grant ignored with a warning, revoke, only cannot allow, broken file, `.rune` as a file |
| policy-gates-hook-handlers | A Session Switch Turns Names Off For One Session | `src/policy/tests.rs`: this session, another session, a missing file, an unsafe id |
| policy-gates-hook-handlers | The Owner Checks Names Before An Event | `src/cli/policy.rs` tests and `tests/hook_policy.rs`: the line format and the exit codes |
| policy-gates-hook-handlers | The Hook List Shows Each Policy Name | `src/cli/hook/mod.rs` tests: the column, and the unchanged plan. `tests/hook_policy.rs`: the column from the real binary. `hooks.rs` tests: the name in the plan, absent when none, and the name check |
