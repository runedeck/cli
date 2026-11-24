# Ask questions about text: design

## Approach

`rune ask` is its own module beside `run` and `launch`. It reads the `ask:` section of the configuration, resolves one route, builds one HTTP request from the question set and the state, sends it, and returns an `Outcome`. The outcome goes through the runner's one printer, so `--json`, the text form, and the exit code table are shared with `rune run` and not a second implementation. The module reuses the launcher's redaction helpers, the env-file reader, and the model-list check, and nothing of its route resolution: a judging model has no profile, no binary, no middleware, and no process environment.

Two alternatives lost. Keeping `rune run jev` as a model tool bent the runner's flags and help around a tool that is not a harness, and routed through a profile mechanism built for processes. A `rune ask` that resolved its route through the launcher would carry the same bend one level down.

The route table is the `ask:` section. `ask.model` names the default route, `jev` unless set. `ask.questions` lists the question folders, by default the owner's checkout. `ask.routes.<name>` holds `id`, `base_url`, and `key_env`. The route `jev` is built in: `jev-1.13.0`, `https://api.typesafe.ai`, `JEV_API_KEY`, and `ask.routes.jev` overrides any field of it. Any other route needs a `base_url`, defaults its `id` to its name, and sends no authorization header when it names no `key_env`. `--model` names a route, or, when no route has that name, a model id on the default route.

The key is read at call time: the process environment, then the env file the configuration names. The value joins the redaction set before the request goes out. A missing key is a configuration error with exit 125, and nothing is sent. A route without a key variable is never missing one.

A question set is read as JSON. The command takes the file's `questions` member and sends that object alone. The `signal` member, and anything else in the file, stays out of the request. A file without a `questions` object is refused.

## Structure

| Component | Change |
| --- | --- |
| `cli::ask` | the command: route, question set, state, request, reply, text and JSON forms, plan, check |
| `cli::ask::not_a_harness` | the one-line refusal `rune run jev` and `rune launch jev` print |
| `ontology::Ask`, `AskRoute` | the `ask:` section: `model`, `questions`, `routes.<name>.{id, base_url, key_env}` |
| `ontology::LaunchTool` | `key_env` and `questions` leave, `binary` and `base_url_env` stay |
| `launch::lookup_env_reference` | one variable from the process environment, else the env file |
| `run::RunOptions` | `questions` and `state_file` leave |
| `cli::Command::Ask` | the clap surface, with a usage error at 125 as for `run` |
| `tests/ask.rs` | the refusals, the text sources, a configured route, and the config keys end to end |

The request is `{"model", "state", "questions"}` posted to `<base>/v1/systemone`, or `<base>/systemone` when the base already ends in `/v1`, with `Authorization: Bearer <key>` when the route names a key variable. The reply's `answers` object is the result. `usage` and `model` are passed through when the reply carries them. A reply without an `answers` object is an endpoint error.

The text form prints one line per answer, sorted by question key: `<key>: <type> <value>` when the answer names its type and carries a scalar under that name (`mood: noul 0.83`), else `<key>: <answer as JSON>`. The JSON form carries the answers object whole, with `route`, `model`, `usage`, and `details` (`endpoint`, `questions`, `state_source`, `duration_ms`).

Exit codes follow the runner's table. A reply with a status other than 200, a transport failure, or a reply without answers is `endpoint_error` with exit 1. A request past its deadline is `timeout` with exit 124. The default deadline is 60 seconds, and `--timeout` replaces it. A missing key, a question set that cannot be found or read, a state file that is not a JSON object, text beside a state file, or a usage error is a configuration error with exit 125. The check uses the launch check's codes: 0 served, 1 missing, 2 endpoint failure.

The dry run prints the route, the endpoint, the model with its source (`built-in`, `config`, or `argument`), the question-set path, the state source (`<text>`, `<stdin>`, the text file, or the state file), the key variable with `<redacted>` or `unset` (or `none` for a route without one), and the timeout, and sends nothing. The check sends nothing but the model-list request.

## Commands

| Role | Command |
| --- | --- |
| A pipeline step asks one question set about a text | `rune ask session-shape "<state text>"` |
| A step with the text in a file, or on stdin | `rune ask session-shape --file state.txt`, `... \| rune ask session-shape` |
| A caller with a state object of its own | `rune ask ./questions/v2.json --state-file state.json --json` |
| The owner checks the route before a batch | `rune ask --check`, `rune ask --model local --check` |
| The owner routes Jev through a proxy by configuration alone | `ask.routes.jev.base_url: https://cliproxy.internal`, `ask.routes.jev.key_env: CLIPROXY_API_KEY` |
| The owner adds a local System One | `ask.routes.local: {id: gemma3, base_url: http://127.0.0.1:11434}`, then `--model local` or `ask.model: local` |
| The owner names other question folders | `ask.questions: [~/Developer/runedeck/jev/questions, ~/Agents/questions]` |
| Someone types the old command | `rune run jev ...` and `rune launch jev` print `jev is a judging model, not a harness; use rune ask <QUESTIONS> [TEXT]` |

## Risks

- A hosted request costs money and carries the state text. The command sends only what the caller gave as the state and the file's `questions` object. A test checks that `signal` never reaches the wire, and the plan and the check send no state.
- A proxy that answers 200 with a body that is not System One's shape is read as an endpoint error, not as answers. The error names the endpoint and the first line of the body, redacted.
- The default question folder is the owner's checkout path. A machine without it gets a configuration error that names the folders searched, and `ask.questions` points elsewhere.
- A `--model` value that is a typo of a route name is read as a model id on the default route and reaches the endpoint, which refuses it. The dry run shows `model_source: argument` for such a value.
- The launch section ignores unknown keys, so `launch.tools.<name>.key_env` and `questions` from the previous change load and do nothing. The `ask:` section refuses unknown keys, so a key value put there by mistake is named and never read.
