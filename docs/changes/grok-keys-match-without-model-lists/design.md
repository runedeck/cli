# Grok keys match without model lists: design

## Approach

A flat walk of the `model` table beat a list of known ids. The walk joins the path of nested tables with dots and stops at each table that holds `env_key`, so an id written unquoted resolves like a quoted one, and a new version needs no code change. A table that holds `env_key` and a nested table yields both entries.

The guess for a model without an entry has three rungs, and each is safe by construction. When every entry names one variable, there is nothing to choose, so that variable is sent. When entries name different variables, the entry with the longest shared id prefix is the model's nearest relative, but only when the prefix reaches past the model's family (the id up to its first digit, `grok` for `grok-4.7`), because `gpt-5` and `grok-4` share a `g` and nothing else. A tie between entries that name different variables sends nothing, because either key could be the wrong one, and that was a review finding. A tie between entries that name the same variable is no tie: the choice is the same either way.

The check says what it did. A guess or an absence carries its reason beside the credential, so a 401 after a guess reads as a wrong guess and not as a bad key. The reason names model ids and variable names, never a value.

The suggestion for a missing model uses the same family and prefix rule over the served list, and on a tie the greatest id, which is the newest version in the usual naming. It is a hint in the report and changes no exit code.

## Structure

- `src/cli/surface/grok_keys.rs` (new): the flat entries, the choice with its reason, and the family and prefix helpers, with unit tests for the nested table, the one-variable file, the longest prefix, the tie, and the family floor.
- `src/cli/surface/mod.rs`: `grok_model_env_key` returns the choice instead of an optional name.
- `src/cli/launch/check/mod.rs`: the route-named variables come with the note; `EndpointReport.credential_note` and `EndpointResult::Listed.closest`, both left out of the JSON when empty; the text lines.
- `tests/run.rs`: the check with a nested entry, a guessed model, a model of another family, and the suggestion line.

## Risks

- A guess sends a real key to an endpoint for a model the owner did not map. The rungs limit it to the one variable of the file or the nearest relative, the report says it was a guess, and a tie or a foreign family sends nothing.
- The JSON report gains fields. They are left out when empty, so an existing consumer sees the same objects it did.
