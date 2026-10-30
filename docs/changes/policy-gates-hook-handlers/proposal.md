---
adr: docs/changes/policy-gates-hook-handlers/adr.md
status: proposed
decisions: ["The hook dispatcher reads the agent policy before it starts a handler"]
---

# Policy gates hook handlers

## Why

The agent policy at `~/.config/rune/policy.yaml` says what agents may do in each place. Today only the Jev handler reads it, because Jev carries its own reader. Every other handler runs wherever the dispatcher fires. The owner cannot turn a handler off in one repository, in one directory tree, or for one session without editing the hook config and reinstalling the harness tables. A second copy of the reader in every handler would drift from the first.

## What Changes

- A handler in the `hooks:` config may carry `policy: <name>`. Before the dispatcher starts such a handler, it asks the policy whether the name is allowed at the event's working directory for the event's session. A refused handler does not start. The dispatcher log and the hook record say `skipped by policy: <name> (<layer>)`. A handler without the key runs as today.
- Rune gains a policy reader in Rust with the semantics of the Jev reader: the top-level `grant`, the scopes from the outermost to the innermost, `disabled`, and `RUNE_POLICY_DISABLE`. The Jev test cases are ported, so both readers give the same answer.
- The reader adds two layers that both readers share. A repository may carry `.rune/policy.yaml` at its root. That file is read after every scope of the user file and may only take away. A session switch file under the rune state directory lists names that stay off for one session id.
- `rune policy check [--path <dir>] [--session <id>] <name>...` prints one line per name in the format of `jev policy` and exits 1 when any name is refused.
- `rune hook list` shows the policy name of each handler in a column.

## Capabilities

### New Capabilities

- `policy-gates-hook-handlers`: the policy reader and its layers, the dispatcher's skip, the owner's check, and the column in the hook list.

### Modified Capabilities

None. The dispatcher's order, budget, failure policy, and result shape stay as `one-dispatcher-runs-every-hook` states them. A skipped handler counts as neither a run nor a failure.

## Impact

- `src/policy/` (new: the reader, the paths, the layers, the tests), `src/lib.rs`, `src/hooks.rs` (`Handler.policy`, `PlannedHandler.policy`, validation), `src/cli/hook/run.rs` (the skip), `src/cli/hook/mod.rs` (the column), `src/cli/hook/record/mod.rs` (the `skipped` field comment), `src/cli/policy.rs` (new: `rune policy check`), `src/cli/mod.rs` (routing, root help), `tests/fixtures/root-help.txt`, `tests/hook_policy.rs` (new), `tests/support/mod.rs` (the policy variables a test never inherits).
- The Jev handler keeps its own reader and its own checks. The owner's config does not change. The owner adds `policy:` to a handler when the owner wants it.
- Records: `adr.md` in this change. Related: `one-dispatcher-runs-every-hook`, `dispatcher-records-every-hook-execution`.
