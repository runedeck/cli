---
type: proof
change: sample-change-one
recorded: 2026-09-24
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
  - scenario: sample-capability-one#thing-holds/version-prints
    kind: unproven
  - scenario: sample-capability-one#thing-holds/missing-tag-fails
    kind: unproven
---

# Proof: sample-change-one

## sample-capability-one#thing-holds/version-prints

```console
$ rune --version
rune [..] built [..]
```

## sample-capability-one#thing-holds/missing-tag-fails

```console
$ nonesuch --flag
```
