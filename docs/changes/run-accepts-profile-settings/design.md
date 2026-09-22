# Run accepts profile settings: design

## Approach

The refusal in `reject_owned_args` becomes a filter, `filter_profile_args`, that returns the arguments to keep and one warning per drop. The owned lists, unchanged in content, move from the five `invoke_*` functions into `owned_options(surface)`. `invoke_surface` filters once, prints the warnings, and hands the surfaces an invocation whose `extra_args` are already clean. It beat a per-surface rewrite, which would have kept five copies of the same loop, and a force flag, which the ADR rejects.

## Structure

- `filter_profile_args(invocation, owned, keyed)` walks `extra_args`. It recognizes `--flag=value`, `-fvalue`, and `--flag value`. An owned flag that takes a value drops the next argument too. `option_takes_value` lists the boolean flags. For each owned argument it asks the surface's `keyed` hook: `None` drops the argument whole with the generic warning, `Some((None, notes))` drops it with the hook's warnings, `Some((Some(replacement), notes))` keeps the flag with a rewritten value.
- `codex_config_key` parses `key=value`, drops `model`, `model_provider`, `sandbox_mode`, `approval_policy`, `cwd`, keeps the rest.
- `claude_settings_key` parses the JSON object, removes `permissions`, `hooks`, `allowedTools`, `disallowedTools`, `enabledPlugins`, `model`, keeps `env` and unknown keys, drops the flag when nothing is left or the value is not an object.
- `drop_whole` is the hook for grok, agy, and opencode.
- `FilteredArgs { kept, warnings }` is public to the crate so `rune run --dry-run` and the tests read it.
- Tests: whole-flag drop with warning, value-taking flag drops its value, codex config keeps the reasoning effort and drops the model, claude settings keeps `env` and drops `permissions`, settings with only owned keys drops whole, unowned args pass untouched, and the existing bypass table now asserts the drop for every surface.

## Risks

- A flag the run sets but the table does not list passes through. The bypass table test covers the dangerous ones per surface. A new surface flag needs a table entry and a test row.
- Dropping `--config model=` silently changes behavior for a profile that relied on it under `rune run`. The run always sets `-m` from the route, so the dropped value was never in effect, and the warning says so.
- The claude `--settings` filter keeps unknown keys. A future Claude Code setting that widens access would pass until it is listed. The list is the boundary, and it is short on purpose.
