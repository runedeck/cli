---
type: proof
change: queue-reaches-a-running-session
head: 210d506d8bbdfba746c0ef369a810ff5198f9c2e
recorded: 2026-10-25
transcript: 2466b1ebb277dc26163029f44d39e7c375d338d3ee8118d1487921ec64f6209e
scenes:
- scenario: queue-messages-for-sessions#every-message-is-recorded-and-nothing-is-removed/message-for-a-claude-session-is-read-once
  kind: check
- scenario: queue-messages-for-sessions#a-message-waits-for-the-session-s-next-turn/codex-session-takes-a-queued-message
  kind: check
- scenario: queue-messages-for-sessions#a-message-waits-for-the-session-s-next-turn/profile-picks-the-codex-home
  kind: check
- scenario: queue-messages-for-sessions#a-session-reads-its-messages-through-a-hook-or-by-asking/hook-keeps-a-codex-turn-going
  kind: check
- scenario: queue-messages-for-sessions#a-session-reads-its-messages-through-a-hook-or-by-asking/codex-prompt-cannot-take-context
  kind: check
- scenario: queue-messages-for-sessions#listing-changes-nothing/list-after-one-read
  kind: check
- scenario: queue-messages-for-sessions#the-result-reports-the-message/route-that-names-no-harness
  kind: check
- scenario: queue-messages-for-sessions#the-result-reports-the-message/harness-knows-no-such-session
  kind: check
---

# Proof: queue-reaches-a-running-session

One scene per scenario, in closure order. Every scene gives rune a HOME of its own, `home`, so the postbox is `home/.rune/postbox.tsv` and the owner's ledger is never touched. A Codex scene writes a fake `codex` that records its arguments and prints the receipt line of Codex 0.154.0, and runs it with `--binary`, so no Codex app server is reached. Each post names its sender with `--from`, so the recorder's own harness session does not show. Message ids and timestamps change on every run and are elided. `mlr` reads the ledger as the tab-separated table it is.

## queue-messages-for-sessions#every-message-is-recorded-and-nothing-is-removed/message-for-a-claude-session-is-read-once

```console
$ HOME=home rune queue post claude --session 42e43c0b --from codex:01a0abff "Is the deck ready?"
posted message [..] for claude session 42e43c0b in home/.rune/postbox.tsv
$ HOME=home rune queue read --session 42e43c0b
Message [..] from codex:01a0abff at [..]:
Is the deck ready?
$ HOME=home rune queue read --session 42e43c0b
no messages for 42e43c0b
$ mlr --itsv --opprint cut -o -f state,sender,harness,session home/.rune/postbox.tsv
state  sender         harness session
posted codex:01a0abff claude  42e43c0b
read   codex:01a0abff claude  42e43c0b
```

## queue-messages-for-sessions#a-message-waits-for-the-session-s-next-turn/codex-session-takes-a-queued-message

```console
$ sh -s
< cat > fake-codex <<'EOF'
< #!/bin/sh
< for argument in "$@"; do printf '%s\n' "$argument"; done > args.txt
< echo 'Queued message msg-41 for thread 019a-ouroboros.'
< EOF
< chmod +x fake-codex
$ HOME=home rune queue post codex --session ouroboros --from claude:42e43c0b --binary ./fake-codex "Is the deck ready?"
queued message msg-41 for codex thread 019a-ouroboros
$ cat args.txt
queue
--thread=ouroboros
--message=Is the deck ready?
--remote=unix://
$ mlr --itsv --opprint cut -o -f state,sender,harness,session home/.rune/postbox.tsv
state  sender          harness session
posted claude:42e43c0b codex   ouroboros
queued claude:42e43c0b codex   ouroboros
```

## queue-messages-for-sessions#a-message-waits-for-the-session-s-next-turn/profile-picks-the-codex-home

```console
$ sh -s
< mkdir -p home/.config/rune
< printf 'launch:\n  profiles:\n    codex:\n      deep:\n        env:\n          CODEX_HOME: /owner/codex-deep\n        args: [-m, gpt-5.6-sol]\n' > home/.config/rune/config.yaml
< cat > fake-codex <<'EOF'
< #!/bin/sh
< for argument in "$@"; do printf '%s\n' "$argument"; done > args.txt
< printf 'CODEX_HOME=%s\n' "$CODEX_HOME" > env.txt
< echo 'Queued message msg-42 for thread 019a-ouroboros.'
< EOF
< chmod +x fake-codex
$ HOME=home rune queue post deep@codex --session ouroboros --from claude:42e43c0b --binary ./fake-codex "Is the deck ready?"
warning: the profile's arguments start a session and are not passed to the queue: -m gpt-5.6-sol
queued message msg-42 for codex thread 019a-ouroboros
$ cat env.txt args.txt
CODEX_HOME=/owner/codex-deep
queue
--thread=ouroboros
--message=Is the deck ready?
--remote=unix://
```

## queue-messages-for-sessions#a-session-reads-its-messages-through-a-hook-or-by-asking/hook-keeps-a-codex-turn-going

```console
$ HOME=home rune queue post codex --postbox --session 01a0abff --from claude:42e43c0b "Is the deck ready?"
posted message [..] for codex session 01a0abff in home/.rune/postbox.tsv
$ HOME=home rune hook adapter postbox
< {"v":1,"harness":"codex","event":"turn.finish","native_event":"Stop","session_id":"01a0abff","raw":{}}
{"v":1,"effect":"continue_turn","reason":"Messages from other sessions arrived through the rune postbox.[..]from claude:42e43c0b at [..]Is the deck ready?","input":null,"context":[]}
$ HOME=home rune queue list --session 01a0abff
[..]  read    claude:42e43c0b -> codex:01a0abff  Is the deck ready?
```

## queue-messages-for-sessions#a-session-reads-its-messages-through-a-hook-or-by-asking/codex-prompt-cannot-take-context

```console
$ HOME=home rune queue post codex --postbox --session 01a0abff --from claude:42e43c0b "Is the deck ready?"
posted message [..] for codex session 01a0abff in home/.rune/postbox.tsv
$ HOME=home rune hook adapter postbox
< {"v":1,"harness":"codex","event":"prompt.before","native_event":"UserPromptSubmit","session_id":"01a0abff","raw":{}}
{"v":1,"effect":"pass","reason":null,"input":null,"context":[]}
$ HOME=home rune queue list --session 01a0abff
[..]  posted  claude:42e43c0b -> codex:01a0abff  Is the deck ready?
```

## queue-messages-for-sessions#listing-changes-nothing/list-after-one-read

```console
$ HOME=home rune queue post agy --session seat-a --from council first
posted message [..] for agy session seat-a in home/.rune/postbox.tsv
$ HOME=home rune queue post agy --session seat-b --from council second
posted message [..] for agy session seat-b in home/.rune/postbox.tsv
$ HOME=home rune queue read --session seat-a
Message [..] from council at [..]:
first
$ sh -c 'shasum home/.rune/postbox.tsv > before.sum'
$ HOME=home rune queue list
[..]  read    council -> agy:seat-a  first
[..]  posted  council -> agy:seat-b  second
$ shasum -c before.sum
home/.rune/postbox.tsv: OK
```

## queue-messages-for-sessions#the-result-reports-the-message/route-that-names-no-harness

```console
$ HOME=home rune queue post ollama --session seat-a hi
? 125
fatal: cannot post a message for a session on ollama: rune drives no such harness. Harnesses: claude, codex, agy, grok, opencode
$ sh -c 'test ! -e home/.rune/postbox.tsv && echo "no postbox written"'
no postbox written
```

## queue-messages-for-sessions#the-result-reports-the-message/harness-knows-no-such-session

```console
$ sh -s
< cat > fake-codex <<'EOF'
< #!/bin/sh
< echo "Error: No active session found matching 'ouroboros'." >&2
< exit 1
< EOF
< chmod +x fake-codex
$ HOME=home rune --json queue post codex --session ouroboros --from claude:42e43c0b --binary ./fake-codex "Is the deck ready?"
? 1
{"details":{"exit_code":1,"harness_errors":[],"stderr_tail":"Error: No active session found matching 'ouroboros'.","stdout_tail":""},"exit_code":1,"hint":null,"kind":"exit","ok":false,"tool":"codex"}
$ sh -c 'test ! -e home/.rune/postbox.tsv && echo "nothing recorded"'
nothing recorded
```
