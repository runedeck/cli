---
type: proof
change: ask-questions-about-text
head: ec698b09ab458fb64601628718b8cc9c2f7afe91
head_change: prqzylprnnowqppkvluqwuqsnvrpypmt
head_tree: 2a6e013c92c25521d64e21fc838edb8b4313c960
recorded: 2026-11-02
transcript: 9ef69396c388abf3ec19aa51d40947e544411a69cc20a47da57fed101fd8fd98
scenes:
- scenario: ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/text-becomes-the-state
  kind: check
- scenario: ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/text-from-a-file-or-a-stream
  kind: check
- scenario: ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/a-state-object-of-the-caller-s-own
  kind: check
- scenario: ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/unknown-question-set
  kind: check
- scenario: ask-questions-about-text#only-the-questions-leave-the-machine/signal-stays-home
  kind: check
- scenario: ask-questions-about-text#only-the-questions-leave-the-machine/plan-and-check-send-no-state
  kind: check
- scenario: ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/built-in-route-without-configuration
  kind: check
- scenario: ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/proxy-by-configuration-alone
  kind: check
- scenario: ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/another-route
  kind: check
- scenario: ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/harness-commands-refuse-the-judging-model
  kind: check
- scenario: ask-questions-about-text#the-key-is-read-at-call-time-and-never-printed/missing-key
  kind: check
- scenario: ask-questions-about-text#the-key-is-read-at-call-time-and-never-printed/refusal-echoes-the-key
  kind: check
- scenario: ask-questions-about-text#the-reply-is-the-result-with-the-runner-s-exit-codes/answers-as-a-machine-readable-result
  kind: check
- scenario: ask-questions-about-text#the-reply-is-the-result-with-the-runner-s-exit-codes/endpoint-failure
  kind: check
- scenario: ask-questions-about-text#the-reply-is-the-result-with-the-runner-s-exit-codes/deadline-passed
  kind: check
---

# Proof: ask-questions-about-text

One scene per scenario, in closure order. No scene reaches the hosted endpoint: every request goes to a System One stub under `fixtures/`, a local HTTP server the scene starts on a free port. The stub answers `POST /v1/systemone` with fixed, typed answers (or the status a scene asks for) and `GET /v1/models` with a model list, and it records each request with the reply it gave as one line of `requests.jsonl`, so a scene can show what left the machine. Every scene gives `rune` a home of its own (`HOME=home`, written by `fixtures/home.sh` with the `ask:` section the scene needs and the `session-shape` question set, which holds a `signal` block), so the owner's config and env file are never read. `env -u JEV_API_KEY` (and `-u CLIPROXY_API_KEY` where that variable matters) drops any key the runner's environment carries, and the keys the scenes use are fakes such as `proof-fake-key`. The plan (`--dry-run`) sends nothing, so the scenes that only plan a route to `api.typesafe.ai` or `cliproxy.internal` never contact them.

Every `rune` command runs through `fixtures/status.sh`, which runs its arguments, prints the real exit status as `exit N`, and exits with it, so the transcript shows the status beside the `? N` line the runner enforces. The first scene prints the path and sha256 of the `rune` binary every scene runs.

## ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/text-becomes-the-state

The text argument becomes `state.text`, the request carries the set's questions whole, and the answers print one per line sorted by question key, although the stub answered `shape` before `mood`.

```console
$# the binary every scene runs: the one the runner put first on PATH
$ sh -c 'rune_path=$(command -v rune); echo "rune: $rune_path"; echo "sha256: $(shasum -a 256 "$rune_path" | cut -d" " -f1)"; rune --version'
rune: [..]/rune
sha256: [..]
rune 0.6.0 ([..]) built [..]
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape "The afternoon went by in one sitting."
mood: noul 0.83
shape: choice one sitting
exit 0
$# the whole request body the stub received
$ jq -c '.body' requests.jsonl
{"model":"jev-1.13.0","questions":{"mood":{"text":"Is the state calm?","type":"noul"},"shape":{"options":["one sitting","broken up"],"text":"How did the session go?","type":"choice"}},"state":{"text":"The afternoon went by in one sitting."}}
$# the question set on disk, for comparison: the questions object is the same, the signal block is not sent
$ jq -c '.questions' questions/session-shape.json
{"mood":{"type":"noul","text":"Is the state calm?"},"shape":{"type":"choice","text":"How did the session go?","options":["one sitting","broken up"]}}
$# what the stub answered, in its own order
$ jq -c '.reply.answers | keys_unsorted' requests.jsonl
["shape","mood"]
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```

## ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/text-from-a-file-or-a-stream

The plan names the text file, or the stream, as the state source. The same stream sent for real reaches the stub as the state's text, and the stream is consumed: a `cat` that shares the same standard input afterwards finds nothing left.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh -c 'printf "The afternoon went by in one sitting.\n" > state.txt'
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape --file state.txt --dry-run
route: jev
endpoint: http://127.0.0.1:[..]/v1/systemone
model: jev-1.13.0
model_source: built-in
questions: questions/session-shape.json
state: state.txt
key: JEV_API_KEY=unset
timeout: 60000ms
exit 0
$# the text on a noninteractive stream
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape --dry-run
< The afternoon went by in one sitting.
route: jev
endpoint: http://127.0.0.1:[..]/v1/systemone
model: jev-1.13.0
model_source: built-in
questions: questions/session-shape.json
state: <stdin>
key: JEV_API_KEY=unset
timeout: 60000ms
exit 0
$# the two plans sent nothing; the stream sent for real is the state's text, and nothing of it is left for the cat that follows
$ sh -c 'env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape; echo "exit $?"; echo "left on stdin:"; cat'
< The afternoon went by in one sitting.
mood: noul 0.83
shape: choice one sitting
exit 0
left on stdin:
$ jq -c '.body.state' requests.jsonl
{"text":"The afternoon went by in one sitting.\n"}
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```

## ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/a-state-object-of-the-caller-s-own

With `--state-file` the request's state is the file's object, whole, and no text is read: a line waits on standard input, the command never consumes it, and a `cat` that shares the same standard input afterwards still reads it whole.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh -c 'printf %s "{\"text\": \"one sitting\", \"hour\": 15}" > state.json'
$# the question set as a path this time, and the result as JSON; the cat afterwards reads the same standard input
$ sh -c 'env HOME=home JEV_API_KEY=proof-fake-key rune ask questions/session-shape.json --state-file state.json --json; echo "exit $?"; echo "still on stdin:"; cat'
< this line is never read
{"answers":{"mood":{"noul":0.83,"type":"noul"},"shape":{"choice":"one sitting","type":"choice"}},"details":{"duration_ms":[..],"endpoint":"http://127.0.0.1:[..]/v1/systemone","questions":"questions/session-shape.json","state_source":"state.json"},"exit_code":0,"kind":"success","model":"jev-1.13.0","ok":true,"route":"jev","usage":{"question_tokens":9,"state_tokens":12},"warnings":[]}
exit 0
still on stdin:
this line is never read
$ jq -c '.body.state' requests.jsonl
{"hour":15,"text":"one sitting"}
$ jq -c '.body | tostring | contains("never read")' requests.jsonl
false
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```

## ask-questions-about-text#a-caller-asks-a-question-set-about-one-piece-of-state/unknown-question-set

A name no configured folder holds exits 125 with a configuration error that names the name and every folder searched, in text and as JSON.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/home.sh "questions: [questions, more-questions]"
ask:
  questions: [questions, more-questions]
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask nowhere "The afternoon went by in one sitting." --dry-run
? 125
fatal: no question set 'nowhere' (nowhere.json) in questions, more-questions
exit 125
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask nowhere "The afternoon went by in one sitting." --dry-run --json
? 125
{"details":{"message":"no question set 'nowhere' (nowhere.json) in questions, more-questions"},"exit_code":125,"kind":"configuration_error","ok":false}
exit 125
```

## ask-questions-about-text#only-the-questions-leave-the-machine/signal-stays-home

The question-set file holds `questions` and `signal`. The request body carries `model`, `state`, and the questions object alone: no object at any depth has a `signal` member. A file without a questions object is refused before anything is sent.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ jq -c '.' questions/session-shape.json
{"questions":{"mood":{"type":"noul","text":"Is the state calm?"},"shape":{"type":"choice","text":"How did the session go?","options":["one sitting","broken up"]}},"signal":{"weight":1,"threshold":0.6,"note":"never leaves the machine"}}
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape "The afternoon went by in one sitting."
mood: noul 0.83
shape: choice one sitting
exit 0
$ jq -c '.body' requests.jsonl
{"model":"jev-1.13.0","questions":{"mood":{"text":"Is the state calm?","type":"noul"},"shape":{"options":["one sitting","broken up"],"text":"How did the session go?","type":"choice"}},"state":{"text":"The afternoon went by in one sitting."}}
$ jq -c '[.body | .. | objects | has("signal")] | any' requests.jsonl
false
$ jq -c '.body | tostring | contains("signal"), contains("threshold"), contains("never leaves")' requests.jsonl
false
false
false
$# a file with the signal block and no questions object is refused, and the stub still saw one request
$ sh -c 'jq -c "{signal: .signal}" questions/session-shape.json > questions/signal-only.json'
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask signal-only "The afternoon went by in one sitting."
? 125
fatal: question set questions/signal-only.json holds no questions object
exit 125
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```

## ask-questions-about-text#only-the-questions-leave-the-machine/plan-and-check-send-no-state

The plan names the endpoint, the model, the question-set path, and the state source, and sends nothing. The check sends one model-list request with no body.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape "The afternoon went by in one sitting." --dry-run
route: jev
endpoint: http://127.0.0.1:[..]/v1/systemone
model: jev-1.13.0
model_source: built-in
questions: questions/session-shape.json
state: <text>
key: JEV_API_KEY=<redacted>
timeout: 60000ms
exit 0
$ sh -c 'echo "requests after the plan: $(wc -l < requests.jsonl | tr -d " ")"'
requests after the plan: 0
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask --check
http://127.0.0.1:[..]/v1/models: jev-1.13.0 served
exit 0
$ jq -c '{method, path, body}' requests.jsonl
{"method":"GET","path":"/v1/models","body":""}
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```

## ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/built-in-route-without-configuration

With no configuration at all, the plan names the built-in model, the hosted endpoint, and the built-in key variable. The question set is given as a path because no folder is configured. The empty home stays empty.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ mkdir empty-home
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=empty-home rune ask fixtures/session-shape.json "The afternoon went by in one sitting." --dry-run
route: jev
endpoint: https://api.typesafe.ai/v1/systemone
model: jev-1.13.0
model_source: built-in
questions: fixtures/session-shape.json
state: <text>
key: JEV_API_KEY=unset
timeout: 60000ms
exit 0
$ ls -A empty-home
```

## ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/proxy-by-configuration-alone

`ask.routes.jev` gives the built-in route another endpoint and another key variable. The plan names them and keeps the built-in model id; with the proxy's key in the environment, the plan hides its value.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: https://cliproxy.internal" "    key_env: CLIPROXY_API_KEY"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: https://cliproxy.internal
      key_env: CLIPROXY_API_KEY
$ sh fixtures/status.sh env -u JEV_API_KEY -u CLIPROXY_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting." --dry-run
route: jev
endpoint: https://cliproxy.internal/v1/systemone
model: jev-1.13.0
model_source: built-in
questions: questions/session-shape.json
state: <text>
key: CLIPROXY_API_KEY=unset
timeout: 60000ms
exit 0
$ sh fixtures/status.sh env -u JEV_API_KEY CLIPROXY_API_KEY=proof-proxy-key HOME=home rune ask session-shape "The afternoon went by in one sitting." --dry-run --json
{"base_url":"https://cliproxy.internal","endpoint":"https://cliproxy.internal/v1/systemone","exit_code":0,"key":"<redacted>","key_env":"CLIPROXY_API_KEY","kind":"dry_run","model":"jev-1.13.0","model_source":"built-in","ok":true,"questions":"questions/session-shape.json","route":"jev","state_source":"<text>","timeout":60000}
exit 0
```

## ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/another-route

A configured route with an endpoint and no key variable, picked with `--model`: the plan says `key: none`, and the request reaches that endpoint with no authorization header.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  local:" "    id: gemma3" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    local:
      id: gemma3
      base_url: http://127.0.0.1:[..]
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting." --model local --dry-run
route: local
endpoint: http://127.0.0.1:[..]/v1/systemone
model: gemma3
model_source: config
questions: questions/session-shape.json
state: <text>
key: none
timeout: 60000ms
exit 0
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting." --model local
mood: noul 0.83
shape: choice one sitting
exit 0
$ jq -c '{method, path, authorization, model: .body.model}' requests.jsonl
{"method":"POST","path":"/v1/systemone","authorization":null,"model":"gemma3"}
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```

## ask-questions-about-text#the-judging-model-is-a-route-not-a-harness/harness-commands-refuse-the-judging-model

`rune run jev` and `rune launch jev` fail with one line that calls it a judging model and names `rune ask`. Nothing starts: stand-ins for `claude`, `codex`, `cursor-agent`, and `opencode` sit first on `PATH` and append a line to `harness.log` whenever one of them runs, and the log is empty after both refusals. The scene's state directory, where a run would leave its record, stays empty too.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/home.sh
ask:
  questions: [questions]
$# the stand-ins resolve first, and running one writes to the log
$ sh -c 'PATH="$PWD/fixtures/bin:$PATH" HARNESS_LOG="$PWD/harness.log" command -v claude codex cursor-agent opencode'
[..]/fixtures/bin/claude
[..]/fixtures/bin/codex
[..]/fixtures/bin/cursor-agent
[..]/fixtures/bin/opencode
$ sh -c 'PATH="$PWD/fixtures/bin:$PATH" HARNESS_LOG="$PWD/harness.log" claude --version; cat harness.log'
claude started with: --version
$# the log emptied; now the refusals, with the same PATH
$ sh -c ': > harness.log'
$ sh -c 'PATH="$PWD/fixtures/bin:$PATH" HARNESS_LOG="$PWD/harness.log" HOME=home rune run jev "The afternoon went by in one sitting." --repo .; echo "exit $?"'
fatal: jev is a judging model, not a harness; use rune ask <QUESTIONS> [TEXT]
exit 125
$ sh -c 'PATH="$PWD/fixtures/bin:$PATH" HARNESS_LOG="$PWD/harness.log" HOME=home rune launch jev; echo "exit $?"'
fatal: jev is a judging model, not a harness; use rune ask <QUESTIONS> [TEXT]
exit 2
$ sh -c 'PATH="$PWD/fixtures/bin:$PATH" HARNESS_LOG="$PWD/harness.log" HOME=home rune run jev "The afternoon went by in one sitting." --repo . --json; echo "exit $?"'
{"details":{"message":"jev is a judging model, not a harness; use rune ask <QUESTIONS> [TEXT]"},"exit_code":125,"kind":"configuration_error","ok":false,"tool":"jev"}
exit 125
$# no stand-in ran, and the state directory holds no record
$ cat harness.log
$ sh -c 'echo "harness starts: $(wc -l < harness.log | tr -d " ")"'
harness starts: 0
$ sh -c 'ls -A "$RUNE_STATE_DIR"'
```

## ask-questions-about-text#the-key-is-read-at-call-time-and-never-printed/missing-key

The route names `JEV_API_KEY`, the variable is unset, and the scene's home has no env file: exit 125 with a configuration error that names the variable and the env file, and the stub saw no request.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting."
? 125
fatal: JEV_API_KEY is unset in the environment and absent from home/.env; export it or add it there
exit 125
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting." --json
? 125
{"details":{"message":"JEV_API_KEY is unset in the environment and absent from home/.env; export it or add it there"},"exit_code":125,"kind":"configuration_error","ok":false}
exit 125
$ sh fixtures/stub.sh stop
stub stopped after 0 requests
```

## ask-questions-about-text#the-key-is-read-at-call-time-and-never-printed/refusal-echoes-the-key

The key comes from the scene's env file at call time. The stub answers 401 with a body that quotes the bearer token. The command exits 1 with an endpoint error, status 401, and the key reads as `<redacted>`; the stub's record shows it received and echoed the key, and the command's JSON never holds it.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start --status 401 --echo-key
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh -c 'printf "JEV_API_KEY=proof-fake-key-7f3a\n" > home/.env'
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting."
? 1
error: http://127.0.0.1:[..]/v1/systemone: HTTP 401 (key refused): {"error": "invalid api key: <redacted>"}
exit 1
$ sh fixtures/status.sh env -u JEV_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting." --json
? 1
{"details":{"endpoint":"http://127.0.0.1:[..]/v1/systemone","message":"http://127.0.0.1:[..]/v1/systemone: HTTP 401 (key refused): {\"error\": \"invalid api key: <redacted>\"}","status":401,"timeout_ms":60000},"exit_code":1,"kind":"endpoint_error","model":"jev-1.13.0","ok":false,"route":"jev"}
exit 1
$# the stub saw the key from the env file and quoted it back
$ jq -cs '.[0] | {authorization, status, reply}' requests.jsonl
{"authorization":"Bearer proof-fake-key-7f3a","status":401,"reply":{"error":"invalid api key: proof-fake-key-7f3a"}}
$# the command's whole output, searched for the key
$ sh -c 'env -u JEV_API_KEY HOME=home rune ask session-shape "The afternoon went by in one sitting." --json 2>&1 | jq -c "tostring | contains(\"proof-fake-key-7f3a\")"'
false
$ sh fixtures/stub.sh stop
stub stopped after 3 requests
```

## ask-questions-about-text#the-reply-is-the-result-with-the-runner-s-exit-codes/answers-as-a-machine-readable-result

The stub answers 200 with an answers object, a usage object, and a model name. The result is one object with `ok` true, `kind` success, the answers, the usage, the model, and exit code 0, and the process exits 0.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape "The afternoon went by in one sitting." --json
{"answers":{"mood":{"noul":0.83,"type":"noul"},"shape":{"choice":"one sitting","type":"choice"}},"details":{"duration_ms":[..],"endpoint":"http://127.0.0.1:[..]/v1/systemone","questions":"questions/session-shape.json","state_source":"<text>"},"exit_code":0,"kind":"success","model":"jev-1.13.0","ok":true,"route":"jev","usage":{"question_tokens":9,"state_tokens":12},"warnings":[]}
exit 0
$# what the stub answered with
$ jq -c '{status, reply}' requests.jsonl
{"status":200,"reply":{"model":"jev-1.13.0","answers":{"shape":{"type":"choice","choice":"one sitting"},"mood":{"type":"noul","noul":0.83}},"usage":{"state_tokens":12,"question_tokens":9}}}
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```

## ask-questions-about-text#the-reply-is-the-result-with-the-runner-s-exit-codes/endpoint-failure

The stub answers 503: exit 1 with kind `endpoint_error` and status 503 in the details.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start --status 503
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape "The afternoon went by in one sitting."
? 1
error: http://127.0.0.1:[..]/v1/systemone: HTTP 503: {"error": "service unavailable"}
exit 1
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape "The afternoon went by in one sitting." --json
? 1
{"details":{"endpoint":"http://127.0.0.1:[..]/v1/systemone","message":"http://127.0.0.1:[..]/v1/systemone: HTTP 503: {\"error\": \"service unavailable\"}","status":503,"timeout_ms":60000},"exit_code":1,"kind":"endpoint_error","model":"jev-1.13.0","ok":false,"route":"jev"}
exit 1
$ sh fixtures/stub.sh stop
stub stopped after 2 requests
```

## ask-questions-about-text#the-reply-is-the-result-with-the-runner-s-exit-codes/deadline-passed

The stub holds its reply for two seconds and the deadline is one: exit 124 with kind `timeout` and the deadline in the details. The stub recorded the request before it slept, so the record shows the request went out whole.

```console
$ sh -c 'cp -R "$RUNE_PROOF_ROOT/docs/proofs/ask-questions-about-text/fixtures" .'
$ sh fixtures/stub.sh start --delay 2
stub listening on 127.0.0.1:[..]
$ sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
ask:
  questions: [questions]
  routes:
    jev:
      base_url: http://127.0.0.1:[..]
$ sh fixtures/status.sh env HOME=home JEV_API_KEY=proof-fake-key rune ask session-shape "The afternoon went by in one sitting." --timeout 1s --json
? 124
{"details":{"endpoint":"http://127.0.0.1:[..]/v1/systemone","message":"http://127.0.0.1:[..]/v1/systemone: no reply within 1000ms","status":null,"timeout_ms":1000},"exit_code":124,"kind":"timeout","model":"jev-1.13.0","ok":false,"route":"jev"}
exit 124
$ jq -c '{method, path, state: .body.state}' requests.jsonl
{"method":"POST","path":"/v1/systemone","state":{"text":"The afternoon went by in one sitting."}}
$ sh fixtures/stub.sh stop
stub stopped after 1 request
```
