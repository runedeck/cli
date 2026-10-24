---
type: proof
change: sample-change-one
recorded: 2026-10-24
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
  - scenario: sample-capability-one#thing-holds/record-lists-nothing
    kind: unproven
---

# Proof: sample-change-one

A scene that reads the hook record without naming a state dir. It lists
nothing, because the runner gives the scene a state dir of its own.

## sample-capability-one#thing-holds/record-lists-nothing

```console
$ rune hook record --json
```
