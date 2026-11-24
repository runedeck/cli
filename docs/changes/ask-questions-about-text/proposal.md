---
status: proposed
---

# Ask questions about text

## Why

Jev is TypeSafe's System One model. It takes a state and a set of typed questions in one request and answers each question with a probability. The owner's Jev pipeline calls it from a client in `~/Developer/runedeck/jev` with its own key handling, its own question-set lookup, and its own output shape. The previous change put that call under `rune run jev`. A judging model is not a harness: it keeps no conversation, runs no process, and takes no prompt. Under `rune run` it borrowed flags the harness tools refuse, a help section of its own, and a route mechanism built for processes. The verb is wrong, so the command moves.

## What Changes

- `rune ask` is a top-level command. It asks a question set about one piece of state and prints typed answers. The text comes from an argument, a file, or stdin and becomes the state's text. A caller that already has a state object gives the whole object instead.
- A question set is a file or a name. A name is looked up in the question folders the configuration lists. The command sends the file's `questions` object alone. The file's `signal` block never leaves the machine.
- The route is model-neutral. `jev` stays a built-in route: `jev-1.13.0` at TypeSafe's hosted endpoint, key in `JEV_API_KEY`. Any route that speaks `POST <base>/v1/systemone` joins by configuration. The settings move from `launch.tools.jev` to a `ask:` section: the default route, the question folders, and per-route id, base URL, and key variable, so a proxy is a configuration change alone.
- The key is read at call time from the variable the route names, from the environment or the env file, never from the configuration itself. The value joins the redaction set before anything is sent.
- `--model`, `--json`, `--dry-run`, `--check`, the exit codes, and the redaction stay as the previous change built them. Under `--json` the result carries `answers`, `model`, `usage`, `ok`, `kind`, and `details`.
- `jev` leaves the `rune run` and `rune launch` tool lists, `--questions` and `--state-file` leave `rune run`, and the model-tool help section goes. `rune run jev` and `rune launch jev` fail with one line that points at `rune ask`.

## Capabilities

- ask-questions-about-text (new)

## Impact

- `src/cli/ask/mod.rs` (new), `src/cli/ask/tests.rs` (new), `src/cli/run/jev/` (removed), `src/cli/mod.rs`, `src/cli/run/mod.rs`, `src/cli/launch/mod.rs`, `src/cli/launch/check/mod.rs`, `src/cli/config/recovery.rs`, `src/ontology.rs`, their tests, `tests/ask.rs` (new), `tests/fixtures/root-help.txt`, `docs/config-reference.json`, `config.example.yaml`, `README.md`, `CHANGELOG.md`.
- A `ask:` configuration section replaces `launch.tools.<name>.key_env` and `launch.tools.<name>.questions`. The launch section ignores unknown keys, so a config file from the previous change still loads and the old keys do nothing.
- Nothing changes for the seven harness tools.
- The Jev client repository keeps its question sets. This change reads them and writes nothing there.
