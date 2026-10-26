---
type: proof
change: benchmark-runner-in-rune
recorded: 2026-10-26
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
- scenario: benchmark-results-stay-compatible#skatebench-compatible-qa-runner/resume-reuses-prior-runs
  kind: unproven
- scenario: benchmark-results-stay-compatible#skatebench-compatible-qa-runner/stale-cache-is-rejected
  kind: unproven
- scenario: benchmark-results-stay-compatible#output-format-parity/cross-runner-diff-is-clean
  kind: unproven
- scenario: benchmark-results-stay-compatible#suite-tiers/bare-suite-name-resolution
  kind: unproven
- scenario: benchmark-results-stay-compatible#model-registry/unknown-model-id
  kind: unproven
- scenario: benchmark-results-stay-compatible#workspace-diagnostics/missing-workspace
  kind: unproven
- scenario: benchmark-results-stay-compatible#coding-tool-models/claude-code-model-receives-the-suite-system-prompt
  kind: unproven
- scenario: benchmark-results-stay-compatible#coding-tool-models/opencode-reply-keeps-the-last-assistant-message
  kind: unproven
---

# Proof: benchmark-runner-in-rune

The scenes prove the native bench runner against the real `rune` binary, and the last two prove the coding tool layer that the runner shares with `rune run`. The fixture `tests/fixtures/benchmark-runner-in-rune/` holds a bench workspace with a committed and a private suite, a registry of an echo model and two coding tool models, and stub `claude` and `opencode` binaries. Its `home.sh` copies them and a rune config that names the workspace into the scene directory, and sets the executable bit on the stubs, so the fixture carries no mode bits. Each scene runs `rune` with `HOME=.`. The coding tool scenes start the stubs from `PATH`, because a real model run costs money and its answer is not deterministic. The parity scene copies the bun harness from `~/Developer/runedeck/bench`, the checkout path that rune discovers, and runs it beside rune.

## benchmark-results-stay-compatible#skatebench-compatible-qa-runner/resume-reuses-prior-runs

The scene marks every cached run as correct by hand. The reused runs keep the score of their text, so the cached flag has no effect.

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/benchmark-runner-in-rune/home.sh"'
$ HOME=. rune bench run --suite skate-basics --models echo
[plan] echo: total 2, reuse 0, execute 2
[done] echo test 1.1 correct in [..]ms
[done] echo test 2.1 incorrect in [..]ms
Results: workspace/bench/results/skate-basics/unversioned/test-results-[..].json
Report:  workspace/bench/results/skate-basics/unversioned/test-results-[..].md
Summary: workspace/bench/results/skate-basics/unversioned/summary-[..].json
$ sh -s
< cd workspace/bench/results/cache/skate-basics/unversioned
< for entry in *.json; do jq '.result.correct = true' "$entry" > "$entry.edit"; mv -f "$entry.edit" "$entry"; done
$ HOME=. rune bench run --suite skate-basics --models echo --runs 2
[plan] echo: total 4, reuse 2, execute 2
[done] echo test 1.2 correct in [..]ms
[done] echo test 2.2 incorrect in [..]ms
Results: workspace/bench/results/skate-basics/unversioned/test-results-[..].json
Report:  workspace/bench/results/skate-basics/unversioned/test-results-[..].md
Summary: workspace/bench/results/skate-basics/unversioned/summary-[..].json
$ sh -s
< cd workspace/bench/results/skate-basics/unversioned
< jq -c '.results[] | select(.result.reused) | {test: (.testIndex + 1), run: .runNumber, correct: .result.correct}' "$(ls test-results-*.json | tail -n 1)"
{"test":1,"run":1,"correct":true}
{"test":2,"run":1,"correct":false}
```

## benchmark-results-stay-compatible#skatebench-compatible-qa-runner/stale-cache-is-rejected

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/benchmark-runner-in-rune/home.sh"'
$ HOME=. rune bench run --suite skate-basics --models echo
[plan] echo: total 2, reuse 0, execute 2
...
$ sh -s
< cd workspace/suites
< jq '.system_prompt = "Name the trick in one word."' skate-basics.json > edited.json
< mv -f edited.json skate-basics.json
$ HOME=. rune bench run --suite skate-basics --models echo
? 2
fatal: Cached entry system prompt mismatch for echo__run1__[..].json. Expected current suite system prompt. Delete 'workspace/bench/results/cache/skate-basics/unversioned' to reset cache.
```

## benchmark-results-stay-compatible#output-format-parity/cross-runner-diff-is-clean

The cache file names carry the signature hash of each test, so a glob on the hash pairs the entries of both runners. The last step diffs each pair byte for byte and counts the changed lines. Each pair differs in its timestamp, and no changed line falls outside the timestamp and timing fields, whose millisecond values can differ from run to run.

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/benchmark-runner-in-rune/home.sh"'
$ sh -s
< harness="$HOME/Developer/runedeck/bench/bench"
< mkdir -p bun
< cp -Rf "$harness/src" bun/src
< cp -f "$harness/package.json" bun/package.json
< ln -s "$harness/node_modules" bun/node_modules
$ bun run bun/src/cli.ts run --suite workspace/suites/skate-basics.json --models echo --config workspace/bench/models.yaml --results bun-results
[plan] echo: total 2, reuse 0, execute 2
[done] echo test 1.1 correct in [..]ms
[done] echo test 2.1 incorrect in [..]ms
Results: bun-results/skate-basics/unversioned/test-results-[..].json
Report:  bun-results/skate-basics/unversioned/test-results-[..].md
Summary: bun-results/skate-basics/unversioned/summary-[..].json
$ HOME=. rune bench run --suite workspace/suites/skate-basics.json --models echo --results rune-results
[plan] echo: total 2, reuse 0, execute 2
[done] echo test 1.1 correct in [..]ms
[done] echo test 2.1 incorrect in [..]ms
Results: rune-results/skate-basics/unversioned/test-results-[..].json
Report:  rune-results/skate-basics/unversioned/test-results-[..].md
Summary: rune-results/skate-basics/unversioned/summary-[..].json
$ sh -s
< for doc in 'skate-basics/unversioned/test-results-*.json' 'skate-basics/unversioned/summary-*.json' 'cache/skate-basics/unversioned/echo__run1__10aa9632c8a5__*.json' 'cache/skate-basics/unversioned/echo__run1__cc8ccedb8b8a__*.json'; do
<   stamps=$(diff bun-results/$doc rune-results/$doc | rg -c '^[<>] +"timestamp": ')
<   others=$(diff bun-results/$doc rune-results/$doc | rg '^[<>] ' | rg -c -v '"(timestamp|duration|averageDuration|tokensPerSecond)": ')
<   echo "$doc: ${stamps:-0} timestamp lines, ${others:-0} other changed lines"
< done
skate-basics/unversioned/test-results-*.json: 2 timestamp lines, 0 other changed lines
skate-basics/unversioned/summary-*.json: 2 timestamp lines, 0 other changed lines
cache/skate-basics/unversioned/echo__run1__10aa9632c8a5__*.json: 2 timestamp lines, 0 other changed lines
cache/skate-basics/unversioned/echo__run1__cc8ccedb8b8a__*.json: 2 timestamp lines, 0 other changed lines
```

## benchmark-results-stay-compatible#suite-tiers/bare-suite-name-resolution

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/benchmark-runner-in-rune/home.sh"'
$ HOME=. rune bench list
enabled   echo  provider=echo  model=echo  runs=1  concurrency=1
disabled  claude-stub  provider=claude-cli  model=sonnet  runs=1  concurrency=1
disabled  opencode-stub  provider=opencode-cli  model=proof/stub  runs=1  concurrency=1

suite  committed skate-basics
suite  private   skate-held-out
$ HOME=. rune bench run --suite s --models echo
? 2
fatal: suite prefix 's' is too short: use at least two characters or the full name
$ HOME=. rune bench run --suite sk --models echo
? 2
fatal: suite prefix 'sk' is ambiguous: skate-basics (committed), skate-held-out (private)
$ HOME=. rune bench run --suite skate-h --models echo
[plan] echo: total 1, reuse 0, execute 1
[done] echo test 1.1 incorrect in [..]ms
Results: workspace/bench/results/skate-held-out/unversioned/test-results-[..].json
Report:  workspace/bench/results/skate-held-out/unversioned/test-results-[..].md
Summary: workspace/bench/results/skate-held-out/unversioned/summary-[..].json
```

## benchmark-results-stay-compatible#model-registry/unknown-model-id

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/benchmark-runner-in-rune/home.sh"'
$ HOME=. rune bench run --suite skate-basics --models nope
? 2
fatal: --models: unknown model id 'nope' (see 'rune bench list')
```

## benchmark-results-stay-compatible#workspace-diagnostics/missing-workspace

The scene runs without the fixture, so `HOME=.` holds no rune config and no checkout at a discovery path.

```console
$ HOME=. rune bench doctor
? 1
 Bench
   ✗ no bench workspace found: add a checkout to `bench` in ~/.config/rune/config.yaml (e.g. `rune config set bench ~/Developer/runedeck/bench`)
$ HOME=. rune bench list
? 2
fatal: no bench workspace found: add a checkout to `bench` in ~/.config/rune/config.yaml [..]
$ HOME=. rune bench run --suite skate-basics
? 2
fatal: no bench workspace found: [..]
$ HOME=. rune bench report --suite skate-basics
? 2
fatal: no bench workspace found: [..]
$ HOME=. rune bench dashboard
? 2
fatal: no bench workspace found: [..]
$ HOME=. rune bench audit
? 2
fatal: no bench workspace found: [..]
```

## benchmark-results-stay-compatible#coding-tool-models/claude-code-model-receives-the-suite-system-prompt

The stub `claude` answers every prompt with a JSON result object whose text is `ollie`, the shape `claude --print --output-format json` prints, and writes the arguments and standard input of its latest call to `claude.log`.

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/benchmark-runner-in-rune/home.sh"'
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. exec rune bench run --suite skate-basics --models claude-stub'
[plan] claude-stub: total 2, reuse 0, execute 2
[done] claude-stub test 1.1 correct in [..]ms
[done] claude-stub test 2.1 incorrect in [..]ms
Results: workspace/bench/results/skate-basics/unversioned/test-results-[..].json
Report:  workspace/bench/results/skate-basics/unversioned/test-results-[..].md
Summary: workspace/bench/results/skate-basics/unversioned/summary-[..].json
$ cat claude.log
arg: --print
arg: --output-format
arg: json
arg: --permission-mode
arg: plan
arg: --tools
arg: Read,Glob,Grep
arg: --allowedTools
arg: Read,Glob,Grep
arg: --model
arg: sonnet
arg: --append-system-prompt
arg: Name the skateboard trick. Reply with the trick name only.
stdin: The front foot flicks off the toe edge and the board spins once along its length. Which trick is it?
```

## benchmark-results-stay-compatible#coding-tool-models/opencode-reply-keeps-the-last-assistant-message

The stub `opencode` prints `heelflip` as its first assistant message and `kick` and `flip` as the two text parts of its last one. Test 2 lists `heelflip` as a negative answer, so it scores as correct only when the runner drops the first message.

```console
$ sh -c 'sh "$RUNE_PROOF_ROOT/tests/fixtures/benchmark-runner-in-rune/home.sh"'
$ sh -c 'PATH="$PWD/bin:$PATH" HOME=. exec rune bench run --suite skate-basics --models opencode-stub'
[plan] opencode-stub: total 2, reuse 0, execute 2
[done] opencode-stub test 1.1 incorrect in [..]ms
[done] opencode-stub test 2.1 correct in [..]ms
Results: workspace/bench/results/skate-basics/unversioned/test-results-[..].json
Report:  workspace/bench/results/skate-basics/unversioned/test-results-[..].md
Summary: workspace/bench/results/skate-basics/unversioned/summary-[..].json
$ sh -c 'jq -c ".results[] | {test: (.testIndex + 1), text: .result.text, correct: .result.correct}" workspace/bench/results/skate-basics/unversioned/test-results-*.json'
{"test":1,"text":"kickflip","correct":false}
{"test":2,"text":"kickflip","correct":true}
```
