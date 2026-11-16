---
type: proof
change: rune-captures-every-session
head: 294f47daddb01ca88b775ed9b87f3b08424d3c4c
head_change: qyxrottqksxutrlwwuwrttttlrlwszps
head_tree: 6689c21b4c8a17b85b2d1959f5d4cf5f8b0dffd1
recorded: 2026-11-16
transcript: e7ff772e29a92753bcc46054f1437d6c6a4855eae00e0a2b681f097b676f6f76
scenes:
- scenario: rune-captures-every-session#native-copies-of-harness-sessions/session-end-copies-the-named-session
  kind: check
- scenario: rune-captures-every-session#work-sessions-stay-out/working-folder-matches-a-glob
  kind: check
- scenario: rune-captures-every-session#supported-sources/source-folder-absent
  kind: check
- scenario: rune-captures-every-session#versions-only-for-lost-bytes/session-grew
  kind: check
- scenario: rune-captures-every-session#versions-only-for-lost-bytes/session-rewritten
  kind: check
- scenario: rune-captures-every-session#capture-debt/session-without-files
  kind: check
---

# Proof: rune-captures-every-session

Six scenes: a capture of one named session, a sweep that leaves an excluded work session out, a sweep on a host with one store, a session that grew, a session that was rewritten, and a session without a file. The scenes run the real `rune session capture`, `rune session sweep`, and `rune session debt`.

Fixture files stand in for the session stores, because the owner's stores are private and a cast is public. Every scene gives rune a HOME of its own, `home`, and a data folder of its own, `data`, so the owner's stores and data folder are never read or written. The first scene elides the end of the capture line: the readable copy runs `specstory` only where it is installed. The other scenarios of the change are checked by `tests/session_capture.rs` and `src/session/tests.rs` and have no scene here.

## rune-captures-every-session#native-copies-of-harness-sessions/session-end-copies-the-named-session

```console
$ sh -s
< mkdir -p home/.claude/projects/-tmp-repo/abc/subagents home/.claude/plans
< printf '{"cwd":"/tmp/repo","type":"user","message":"hi"}\n' > home/.claude/projects/-tmp-repo/abc.jsonl
< printf '{}\n' > home/.claude/projects/-tmp-repo/abc/subagents/one.jsonl
< printf '# plan\n' > home/.claude/plans/plan.md
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session capture --harness claude --session abc --source hook
claude: 2 file(s) copied, 0 version(s) kept, 0 skipped[..]
...
$ sh -c 'find data/Sessions data/Plans -type f | sort'
data/Plans/claude/plan.md
data/Sessions/claude/-tmp-repo/abc.jsonl
data/Sessions/claude/-tmp-repo/abc/subagents/one.jsonl
$ sh -c 'cmp home/.claude/projects/-tmp-repo/abc.jsonl data/Sessions/claude/-tmp-repo/abc.jsonl && echo "the copy equals the session file"'
the copy equals the session file
$ sh -c 'grep -c "hook capture: claude session abc" "$RUNE_STATE_DIR/session-capture.log"'
1
```

## rune-captures-every-session#work-sessions-stay-out/working-folder-matches-a-glob

```console
$ sh -s
< mkdir -p home/.config/rune home/work/app home/.claude/projects/-home-work-app home/.claude/projects/-tmp-own
< printf 'capture:\n  exclude:\n    paths: ["~/work/**"]\n' > home/.config/rune/config.yaml
< printf '{"cwd":"%s/home/work/app","type":"user","message":"work"}\n' "$PWD" > home/.claude/projects/-home-work-app/w1.jsonl
< printf '{"cwd":"/tmp/own","type":"user","message":"kept"}\n' > home/.claude/projects/-tmp-own/kept.jsonl
$ sh -c 'HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/home/.config" DATA_DIR=data rune session sweep'
antigravity: no store on this host
claude: 1 file(s) copied, 0 version(s) kept, 1 skipped
codex: no store on this host
grok: no store on this host
plans 0, backups 0, transcripts 0, debt closed 0, debt open 0
$ sh -c 'find data/Sessions -type f | sort'
data/Sessions/claude/-tmp-own/kept.jsonl
$ sh -c 'HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/home/.config" DATA_DIR=data rune session debt'
no open capture debt
```

## rune-captures-every-session#supported-sources/source-folder-absent

```console
$ sh -s
< mkdir -p home/.claude/projects/-tmp-repo
< printf '{"cwd":"/tmp/repo","type":"user","message":"hi"}\n' > home/.claude/projects/-tmp-repo/abc.jsonl
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session sweep
antigravity: no store on this host
claude: 1 file(s) copied, 0 version(s) kept, 0 skipped
codex: no store on this host
grok: no store on this host
plans 0, backups 0, transcripts 0, debt closed 0, debt open 0
$ sh -c 'find data -type f | sort'
data/Sessions/claude/-tmp-repo/abc.jsonl
```

## rune-captures-every-session#versions-only-for-lost-bytes/session-grew

```console
$ sh -s
< mkdir -p home/.claude/projects/-tmp-repo
< printf '{"cwd":"/tmp/repo","type":"user","message":"first"}\n' > home/.claude/projects/-tmp-repo/abc.jsonl
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session sweep
antigravity: no store on this host
claude: 1 file(s) copied, 0 version(s) kept, 0 skipped
codex: no store on this host
grok: no store on this host
plans 0, backups 0, transcripts 0, debt closed 0, debt open 0
$ sh -s
< printf '{"type":"user","message":"second"}\n' >> home/.claude/projects/-tmp-repo/abc.jsonl
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session sweep
antigravity: no store on this host
claude: 1 file(s) copied, 0 version(s) kept, 0 skipped
codex: no store on this host
grok: no store on this host
plans 0, backups 0, transcripts 0, debt closed 0, debt open 0
$ sh -c 'test ! -e data/SessionVersions && cat data/Sessions/claude/-tmp-repo/abc.jsonl'
{"cwd":"/tmp/repo","type":"user","message":"first"}
{"type":"user","message":"second"}
```

## rune-captures-every-session#versions-only-for-lost-bytes/session-rewritten

```console
$ sh -s
< mkdir -p home/.claude/projects/-tmp-repo
< printf '{"cwd":"/tmp/repo","type":"user","message":"first"}\n' > home/.claude/projects/-tmp-repo/abc.jsonl
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session sweep
antigravity: no store on this host
claude: 1 file(s) copied, 0 version(s) kept, 0 skipped
codex: no store on this host
grok: no store on this host
plans 0, backups 0, transcripts 0, debt closed 0, debt open 0
$ sh -s
< printf '{"cwd":"/tmp/repo","type":"user","message":"rewritten"}\n' > home/.claude/projects/-tmp-repo/abc.jsonl
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session sweep
antigravity: no store on this host
claude: 1 file(s) copied, 1 version(s) kept, 0 skipped
codex: no store on this host
grok: no store on this host
plans 0, backups 0, transcripts 0, debt closed 0, debt open 0
$ sh -c 'cat data/SessionVersions/claude/*/-tmp-repo/abc.jsonl'
{"cwd":"/tmp/repo","type":"user","message":"first"}
$ sh -c 'cat data/Sessions/claude/-tmp-repo/abc.jsonl'
{"cwd":"/tmp/repo","type":"user","message":"rewritten"}
```

## rune-captures-every-session#capture-debt/session-without-files

```console
$ sh -s
< mkdir -p home/.claude/projects/p
< printf '{}\n' > home/.claude/projects/p/x.jsonl
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session capture --harness claude --session probe
claude: no session file
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session debt
no open capture debt
$ HOME=home DATA_DIR=data XDG_CONFIG_HOME=home/.config rune session capture --harness opencode
? 125
rune session: `opencode` is not a captured harness (claude, codex, grok, antigravity)
```
