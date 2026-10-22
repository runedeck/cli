---
type: proof
change: run-returns-harness-artifacts
head: d875d9a012467cfc61a8d56b46328efd4c751f0b
recorded: 2026-10-22
transcript: f5a5818001537d30b11810795b2983ffc5dc3fd7c7984aa6463213149ac11b19
scenes:
- scenario: execute-prompts-off-session#the-run-reports-the-files-the-harness-created/agy-draws-an-image
  kind: check
- scenario: execute-prompts-off-session#the-run-reports-the-files-the-harness-created/grok-answers-in-text
  kind: check
- scenario: execute-prompts-off-session#the-run-reports-the-files-the-harness-created/agy-run-leaves-a-file-from-an-earlier-turn
  kind: unproven
- scenario: execute-prompts-off-session#the-run-copies-the-files-where-the-user-asks/name-clash-in-the-destination
  kind: unproven
- scenario: execute-prompts-off-session#the-run-copies-the-files-where-the-user-asks/dry-run-with-a-destination
  kind: check
- scenario: execute-prompts-off-session#a-clean-run-keeps-its-files/clean-agy-run-draws-an-image
  kind: unproven
---

# Proof: run-returns-harness-artifacts

One scene per scenario, in closure order. agy takes workspace-write mode alone, so every agy scene makes a scratch jj repository under the temporary directory and runs against it. agy signs in through the macOS keychain and Google's endpoints, which the recording sandbox blocks, and grok's own sandbox cannot start inside it: those scenes name the profile they need, and the owner records them on the host with the same fences.

## execute-prompts-off-session#the-run-reports-the-files-the-harness-created/agy-draws-an-image

Needs: gemini@agy. The run reports the image it drew as its one artifact and copies it into `out`.

```console
$ jj git init repo
Initialized repo in "repo"
$ rune --json run gemini@agy --mode workspace-write --repo repo --artifacts-dir out "Draw a tiny icon of a rune stone and save it as rune_icon.jpg. Then reply with the single word done."
...
{"artifacts":["[..]/out/[..]"],[..]"kind":"success",[..]"tool":"agy",[..]}
$ ls out
[..]
```

## execute-prompts-off-session#the-run-reports-the-files-the-harness-created/grok-answers-in-text

Needs: grok@grok. A grok run reports no artifact.

```console
$ rune --json run grok@grok --clean "Reply with exactly two words: grok ready"
...
{"artifacts":[],[..]"text":"grok ready",[..]"tool":"grok",[..]}
```

## execute-prompts-off-session#the-run-reports-the-files-the-harness-created/agy-run-leaves-a-file-from-an-earlier-turn

Needs: gemini@agy, with a file left in `brain/<conversation_id>/` before the run. The unit tests of the artifact listing cover the age rule.

```console
```

## execute-prompts-off-session#the-run-copies-the-files-where-the-user-asks/name-clash-in-the-destination

Needs: gemini@agy. With `rune_icon.jpg` already in `out`, the copy gets a numeric suffix and the first file stays.

```console
$ jj git init repo
Initialized repo in "repo"
$ mkdir out
$ sh -c 'printf "first\n" > out/rune_icon.jpg'
$ rune --json run gemini@agy --mode workspace-write --repo repo --artifacts-dir out "Draw a tiny icon of a rune stone and save it as rune_icon.jpg. Then reply with the single word done."
...
{"artifacts":["[..]/out/rune_icon-1.jpg"],[..]"kind":"success",[..]}
$ cat out/rune_icon.jpg
first
$ ls out
rune_icon-1.jpg
rune_icon.jpg
```

## execute-prompts-off-session#the-run-copies-the-files-where-the-user-asks/dry-run-with-a-destination

The plan prints the destination as `artifacts_dir:`.

```console
$ jj git init repo
Initialized repo in "repo"
$ rune run gemini@agy --mode workspace-write --repo repo --artifacts-dir out --dry-run "x"
tool: agy
...
artifacts_dir: out
...
```

## execute-prompts-off-session#a-clean-run-keeps-its-files/clean-agy-run-draws-an-image

Needs: gemini@agy. A clean run without `--artifacts-dir` keeps the image under the rune state directory.

```console
$ jj git init repo
Initialized repo in "repo"
$ rune --json run gemini@agy --clean --mode workspace-write --repo repo "Draw a tiny icon of a rune stone and save it as rune_icon.jpg. Then reply with the single word done."
...
{"artifacts":["[..]/rune/artifacts/[..]"],[..]"clean":true,[..]"kind":"success",[..]}
```
