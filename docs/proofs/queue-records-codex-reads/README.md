---
type: proof
change: queue-records-codex-reads
head: 2019c64bc794044a8fb9d45518484edf67ca85b5
head_change: wvzquvsnqmpxtsvmlwvuxzqnmzkvuvsn
head_tree: 26119afdb345cf835b9ac60f14a04df6465dc022
recorded: 2026-10-28
transcript: ed2d89946fd00f164d3e4e60a5d4df2654df2bc5928c9b1a88bac62ddc4bd5df
scenes:
- scenario: queue-messages-for-sessions#a-queued-message-is-read-when-its-turn-starts/codex-turn-starts-with-a-queued-message
  kind: check
- scenario: queue-messages-for-sessions#a-queued-message-is-read-when-its-turn-starts/owner-prompt-leaves-the-queue-alone
  kind: check
- scenario: queue-messages-for-sessions#a-codex-session-name-leads-to-its-uuid/refused-name-names-its-uuid
  kind: check
- scenario: queue-messages-for-sessions#a-codex-session-name-leads-to-its-uuid/session-uuid-gets-no-warning
  kind: check
---

# Proof: queue-records-codex-reads

One scene per scenario, in closure order. Every scene gives rune a HOME of its own, `home`, so the postbox is `home/.rune/postbox.tsv` and the owner's ledger is never touched. A Codex scene writes a fake `codex` that prints the receipt line or the name refusal of Codex 0.154.0, and runs it with `--binary`, so no Codex app server is reached. The hook scenes hand `rune hook adapter postbox` the payload the dispatcher builds from Codex's `UserPromptSubmit`, with the prompt. Each post names its sender with `--from`, so the recorder's own harness session does not show. Message ids and timestamps change on every run and are elided. `mlr` reads the ledger as the tab-separated table it is.

## queue-messages-for-sessions#a-queued-message-is-read-when-its-turn-starts/codex-turn-starts-with-a-queued-message

```console
$ sh -s
< cat > fake-codex <<'EOF'
< #!/bin/sh
< echo 'Queued message msg-41 for thread 019a-ouroboros.'
< EOF
< chmod +x fake-codex
$ HOME=home rune queue post codex --session ouroboros --from claude:42e43c0b --binary ./fake-codex "Is the deck ready?"
warning: Codex looks the name ouroboros up page by page across its server's sessions, which can take a minute or more; a session UUID needs no search
queued message msg-41 for codex thread 019a-ouroboros
$ HOME=home rune hook adapter postbox
< {"v":1,"harness":"codex","event":"prompt.before","native_event":"UserPromptSubmit","session_id":"019a-ouroboros","prompt":"Is the deck ready?","raw":{}}
{"v":1,"effect":"pass","reason":null,"input":null,"context":[]}
$ mlr --itsv --opprint cut -o -f state,sender,harness,session home/.rune/postbox.tsv
state  sender          harness session
posted claude:42e43c0b codex   ouroboros
queued claude:42e43c0b codex   019a-ouroboros
read   claude:42e43c0b codex   019a-ouroboros
$ HOME=home rune queue list
[..]  read    claude:42e43c0b -> codex:019a-ouroboros  Is the deck ready?
```

## queue-messages-for-sessions#a-queued-message-is-read-when-its-turn-starts/owner-prompt-leaves-the-queue-alone

```console
$ sh -s
< cat > fake-codex <<'EOF'
< #!/bin/sh
< echo 'Queued message msg-42 for thread 01a15844-7b2e-71a0-aa88-b41f3c4622af.'
< EOF
< chmod +x fake-codex
$ HOME=home rune queue post codex --session 01a15844-7b2e-71a0-aa88-b41f3c4622af --from claude:42e43c0b --binary ./fake-codex "Is the deck ready?"
queued message msg-42 for codex thread 01a15844-7b2e-71a0-aa88-b41f3c4622af
$ HOME=home rune hook adapter postbox
< {"v":1,"harness":"codex","event":"prompt.before","native_event":"UserPromptSubmit","session_id":"01a15844-7b2e-71a0-aa88-b41f3c4622af","prompt":"Run the tests","raw":{}}
{"v":1,"effect":"pass","reason":null,"input":null,"context":[]}
$ HOME=home rune queue list
[..]  queued  claude:42e43c0b -> codex:01a15844-7b2e-71a0-aa88-b41f3c4622af  Is the deck ready?
```

## queue-messages-for-sessions#a-codex-session-name-leads-to-its-uuid/refused-name-names-its-uuid

```console
$ sh -s
< cat > fake-codex <<'EOF'
< #!/bin/sh
< echo 'Error: Cannot verify a unique session label across server pages; matching session UUID: 01a15844-7b2e-71a0-aa88-b41f3c4622af' >&2
< exit 1
< EOF
< chmod +x fake-codex
$ HOME=home rune --json queue post codex --session Proton/ai-demo --from claude:42e43c0b --binary ./fake-codex "Is the deck ready?"
? 1
warning: Codex looks the name Proton/ai-demo up page by page across its server's sessions, which can take a minute or more; a session UUID needs no search
{"details":{"exit_code":1,"harness_errors":[],"stderr_tail":"Error: Cannot verify a unique session label across server pages; matching session UUID: 01a15844-7b2e-71a0-aa88-b41f3c4622af","stdout_tail":""},"exit_code":1,"hint":"Codex matched the name to session 01a15844-7b2e-71a0-aa88-b41f3c4622af: post with --session 01a15844-7b2e-71a0-aa88-b41f3c4622af","kind":"exit","ok":false,"tool":"codex"}
$ sh -c 'test ! -e home/.rune/postbox.tsv && echo "nothing recorded"'
nothing recorded
```

## queue-messages-for-sessions#a-codex-session-name-leads-to-its-uuid/session-uuid-gets-no-warning

```console
$ sh -s
< cat > fake-codex <<'EOF'
< #!/bin/sh
< echo 'Queued message msg-43 for thread 01a15844-7b2e-71a0-aa88-b41f3c4622af.'
< EOF
< chmod +x fake-codex
$ HOME=home rune queue post codex --session 01a15844-7b2e-71a0-aa88-b41f3c4622af --from claude:42e43c0b --binary ./fake-codex "Is the deck ready?"
queued message msg-43 for codex thread 01a15844-7b2e-71a0-aa88-b41f3c4622af
```
