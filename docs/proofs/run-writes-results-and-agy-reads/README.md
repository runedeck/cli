---
type: proof
change: run-writes-results-and-agy-reads
head: 650f94b8b33299a38d21157d849aa7ca92e9c71d
recorded: 2026-10-24
transcript: 5d692d61295c8b78e3d7d7e960ed5b9c2b65f9bea9b60c2c35957c91aed2dd8e
scenes:
- scenario: execute-prompts-off-session#the-runner-writes-its-result-to-a-file-on-request/council-seat-writes-its-answer-to-a-file
  kind: check
- scenario: execute-prompts-off-session#the-runner-writes-its-result-to-a-file-on-request/failed-run-still-writes-its-result
  kind: check
- scenario: execute-prompts-off-session#the-runner-writes-its-result-file-only-in-permitted-places/output-path-in-a-protected-place-is-refused
  kind: check
- scenario: execute-prompts-off-session#the-runner-writes-its-result-file-only-in-permitted-places/caller-names-a-protected-folder-as-the-repository
  kind: check
- scenario: execute-prompts-off-session#the-runner-writes-its-result-file-only-in-permitted-places/argument-error-with-an-output-file
  kind: check
- scenario: execute-prompts-off-session#the-runner-replaces-the-result-file-in-one-step/output-folder-moved-during-the-run
  kind: check
- scenario: execute-prompts-off-session#the-runner-replaces-the-result-file-in-one-step/output-file-already-exists
  kind: check
- scenario: execute-prompts-off-session#the-runner-replaces-the-result-file-in-one-step/result-file-cannot-be-written
  kind: check
- scenario: execute-prompts-off-session#a-read-only-agy-run-is-refused/agy-council-seat-in-read-only-mode
  kind: check
- scenario: execute-prompts-off-session#a-clean-grok-run-keeps-only-the-model-key-map/clean-grok-run-uses-an-api-key-from-the-environment
  kind: check
- scenario: execute-prompts-off-session#a-clean-grok-run-keeps-only-the-model-key-map/grok-echoes-its-key
  kind: unproven
- scenario: execute-prompts-off-session#a-clean-grok-run-keeps-only-the-model-key-map/owner-s-grok-configuration-is-malformed
  kind: unproven
- scenario: execute-prompts-off-session#a-headless-denial-names-its-cause/agy-denies-its-own-shell-command
  kind: unproven
---

# Proof: run-writes-results-and-agy-reads

One scene per scenario, in closure order. The scenes run in a fresh folder under the temporary directory, which is a permitted place for the result file. A scene that needs no model builds a fake harness in that folder and runs the `sol@claude` route with `--binary`. codex and grok start their own sandboxes, which the recording sandbox refuses: those scenes name the profile they need, and the owner records them on the host with the same fences.

## execute-prompts-off-session#the-runner-writes-its-result-to-a-file-on-request/council-seat-writes-its-answer-to-a-file

Needs: sol@codex. The file holds the one JSON object, and stdout holds the confirmation that names the file.

```console
$ rune --json run sol@codex "Reply with exactly two words: seat ready" --output seat.json
{"exit_code":0,"kind":"success","ok":true,"output":"[..]/seat.json"}
$ cat seat.json
{"artifacts":[],[..]"kind":"success",[..]"ok":true,[..]"text":"seat ready",[..]"tool":"codex",[..]}
```

## execute-prompts-off-session#the-runner-writes-its-result-to-a-file-on-request/failed-run-still-writes-its-result

A harness that outlives its deadline: the file holds the timeout object with both stream tails, and the exit code is 124.

```console
$ sh -c 'printf "%s\n" "#!/bin/sh" "echo partial answer" "echo still working >&2" "sleep 5" > slow-claude && chmod 755 slow-claude'
$ rune --json run sol@claude --binary ./slow-claude --timeout 1s "x" --output seat.json
? 124
{"exit_code":124,"kind":"timeout","ok":false,"output":"[..]/seat.json"}
$ cat seat.json
{"details":{[..]"stderr_tail":"still working[..]","stdout_tail":"partial answer[..]"[..]},[..]"kind":"timeout","ok":false,[..]}
```

## execute-prompts-off-session#the-runner-writes-its-result-file-only-in-permitted-places/output-path-in-a-protected-place-is-refused

A path under the home folder, a path inside `.git`, and a symlink are refused before anything starts, with exit 125.

```console
$ sh -c 'rune run sol@claude x --output ~/.claude/settings.json'
? 125
fatal: output file [..]/.claude/settings.json is outside every permitted place: [..]
$ mkdir .git
$ rune run sol@claude x --output .git/hook.json
? 125
fatal: output file [..]/.git/hook.json lies under the hidden folder .git inside [..]
$ sh -c 'printf "{}\n" > real.json && ln -s real.json link.json'
$ rune run sol@claude x --output link.json
? 125
fatal: output file [..]/link.json is a symlink, and the runner replaces only a regular file
```

## execute-prompts-off-session#the-runner-writes-its-result-file-only-in-permitted-places/caller-names-a-protected-folder-as-the-repository

`--repo` never widens the permitted places: the repository root comes from the working directory.

```console
$ sh -c 'rune run sol@claude x --repo ~/.claude --output ~/.claude/settings.json'
? 125
fatal: output file [..]/.claude/settings.json is outside every permitted place: [..]
```

## execute-prompts-off-session#the-runner-writes-its-result-file-only-in-permitted-places/argument-error-with-an-output-file

The usage error prints before the path is accepted, and no file is written.

```console
$ rune run sol@claude x --mode bogus --output seat.json
? 125
error: invalid value 'bogus' for '--mode <MODE>'
  [possible values: read-only, workspace-write, answer-only]

For more information, try '--help'.
$ ls
```

## execute-prompts-off-session#the-runner-replaces-the-result-file-in-one-step/output-folder-moved-during-the-run

The harness moves the output folder away and plants a file at the old path: stdout carries the full result, the exit code is 125, and the planted file is named by nothing.

```console
$ mkdir out
$ sh -c 'printf "%s\n" "#!/bin/sh" "mv out out.old" "mkdir out" "echo planted > out/seat.json" "echo real answer" > mover && chmod 755 mover'
$ rune --json run sol@claude --binary ./mover "x" --output out/seat.json
? 125
error: cannot write result file [..]/out/seat.json: the file at that path is not the file the runner wrote (the folder moved during the run)
{"artifacts":[],[..]"kind":"success",[..]"text":"real answer",[..]}
$ cat out/seat.json
planted
```

## execute-prompts-off-session#the-runner-replaces-the-result-file-in-one-step/output-file-already-exists

A second run replaces the file in one step, and the new file is readable by the owner only.

```console
$ sh -c 'printf "%s\n" "#!/bin/sh" "echo first answer" > one && printf "%s\n" "#!/bin/sh" "echo second answer" > two && chmod 755 one two'
$ rune run sol@claude --binary ./one "x" --output seat.txt
output: [..]/seat.txt
$ cat seat.txt
first answer
$ rune run sol@claude --binary ./two "x" --output seat.txt
output: [..]/seat.txt
$ cat seat.txt
second answer
$ ls -l seat.txt
-rw-------[..] seat.txt
```

## execute-prompts-off-session#the-runner-replaces-the-result-file-in-one-step/result-file-cannot-be-written

The output folder turns read-only while the harness runs: stdout carries the full result object, and the exit code is 125.

```console
$ mkdir out
$ sh -c 'printf "%s\n" "#!/bin/sh" "chmod 555 out" "echo real answer" > locker && chmod 755 locker'
$ rune --json run sol@claude --binary ./locker "x" --output out/seat.json
? 125
error: cannot write result file [..]/out/seat.json: Permission denied[..]
{"artifacts":[],[..]"kind":"success",[..]"text":"real answer",[..]}
$ chmod 755 out
$ ls out
```

## execute-prompts-off-session#a-read-only-agy-run-is-refused/agy-council-seat-in-read-only-mode

```console
$ rune run gemini@agy "Review this repository"
? 125
fatal: agy cannot enforce read-only access, because it writes files in plan mode and ignores deny rules: use --mode workspace-write, which runs agy in its own jj workspace
$ rune run gemini@agy --mode read-only "Review this repository"
? 125
fatal: agy cannot enforce read-only access, because it writes files in plan mode and ignores deny rules: use --mode workspace-write, which runs agy in its own jj workspace
```

## execute-prompts-off-session#a-clean-grok-run-keeps-only-the-model-key-map/clean-grok-run-uses-an-api-key-from-the-environment

Needs: grok@grok. The dry run lists the kept key names and the redacted key variable, and the clean run answers with that key.

```console
$ rune run grok@grok --clean --dry-run "Reply with exactly two words: grok ready"
tool: grok
...
  GROK_CODE_XAI_API_KEY=<redacted>
...
  HOME=<clean-state>
...
clean_config_keys: model.[..].env_key[..]
...
$ rune run grok@grok --clean "Reply with exactly two words: grok ready"
...
grok ready
```

## execute-prompts-off-session#a-clean-grok-run-keeps-only-the-model-key-map/grok-echoes-its-key

Needs: grok@grok. The integration test `grok_echoing_the_value_of_its_key_variable_leaves_no_trace_in_any_output` covers it with a fake harness and a fake key, so no recording asks a model to print a real one.

```console
```

## execute-prompts-off-session#a-clean-grok-run-keeps-only-the-model-key-map/owner-s-grok-configuration-is-malformed

Needs: a malformed `~/.grok/config.toml`. The unit test `a_malformed_grok_configuration_names_the_place_and_quotes_nothing` covers it.

```console
```

## execute-prompts-off-session#a-headless-denial-names-its-cause/agy-denies-its-own-shell-command

Needs: gemini@agy with a prompt that runs a shell command. The integration test of the soft-deny hint covers it with a fake agy log.

```console
```
