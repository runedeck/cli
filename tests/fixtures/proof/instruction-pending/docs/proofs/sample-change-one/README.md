---
type: proof
change: sample-change-one
recorded: 2026-09-24
transcript: b76e4c8110f484cec374acf7273df19465ce28f64e177c26304163e529663a09
scenes:
  - scenario: sample-capability-one#thing-holds/version-prints
    kind: check
  - scenario: sample-capability-one#thing-holds/missing-tag-fails
    kind: instruction
    model: claude-opus-5-5
---

# Proof: sample-change-one

## sample-capability-one#thing-holds/version-prints

```console
$ rune --version
rune [..] built [..]
```

## sample-capability-one#thing-holds/missing-tag-fails

```text
Explain why a missing tag must fail.
```
