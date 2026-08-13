# Adopt Walkthrough

Manual test for the adoption review state machine (`rune adopt`, decisions: CLI-0023 and CLI-0027). The flow: import an upstream artifact, give every block a verdict, let the CLI enforce the verdicts, and seal concise reviewed sidecars.

## Setup

Any directory with a `module.yaml` works as a target. The deck's `runes/meta` already carries `.mdschema` files for skills, agents, and rules; a scratch module needs one beside the kind directory:

```sh
mkdir -p /tmp/claude/adopt-demo/rules
printf 'name: demo\n' > /tmp/claude/adopt-demo/module.yaml
printf 'heading_rules:\n    max_depth: 3\n' > /tmp/claude/adopt-demo/rules/.mdschema
```

## The loop

```sh
# 1. Import + open the session (a local path or a commit-pinned GitHub URL)
rune adopt start file:///path/to/forge-core/rules/AvoidDuplication.md \
    --module /tmp/claude/adopt-demo --kind rule --name avoid-duplication

# 2. See where you stand; both support --json for the skill's context injection
rune adopt status
rune adopt next --count 4

# 3. One verdict per block; adapt and cut demand the rationale
rune adopt verdict avoid-duplication.md:1 keep
rune adopt verdict avoid-duplication.md:2 cut --note "upstream assumption the deck does not share"

# 4. Apply the edits the verdicts imply, then let the CLI check your work
rune adopt finalize
```

A skill tree does the same with `--kind skill` (the default) and a directory source; every companion file joins the session, non-markdown as one block per file.

## What to verify

- [ ] `start` lands adopt/v1 sidecars carrying `review: pending`; `status` and `next` find the temporary session across separate CLI invocations
- [ ] No `review.yaml` or `*.review.yaml` appears in the artifact or its `.provenance` directory
- [ ] `finalize` refuses while any block is pending, and lists the ids
- [ ] Cut a block, finalize WITHOUT editing the file: it refuses with "still appears verbatim"
- [ ] Delete a kept block's text, finalize: it refuses with "kept content missing"
- [ ] After a clean finalize: the sidecar carries `review: reviewed`, reviewer, completion time, concise summary, and the final subject digest; the temporary session is gone
- [ ] `rune adopt start` on the same artifact again: refused ("already passed review")
- [ ] `rune adopt abandon --yes` moves an in-flight adoption and its session to `.trash/`, never deletes the artifact in place
- [ ] `rune provenance`, assembly, and deploy remain unchanged because reviewed sidecars are the authority

## Hardening checks

- [ ] `rune install` over a module with a mid-review adoption skips it by name; the rest deploys; `rune install --strict` fails instead
- [ ] `rune release` and `rune copy` refuse outright while a review is open
- [ ] An adopt sidecar with its `review` field stripped does NOT deploy (fail closed, "adoption without review state")
- [ ] Blocks carrying injection-shaped content arrive flagged from `rune adopt next` (try a paragraph containing "ignore previous instructions"), and `verdict keep` on a flagged block demands `--note`
- [ ] Pending session entries carry `decidedOn` and `transport: verdict-cli`, plus the `lint` and `segmenter` versions; these fields disappear with the session
- [ ] `rune adopt doctor` is clean after a good finalize without any ledger; edit the reviewed file afterwards and it exits 1 naming the tampered subject
- [ ] Doctor reports legacy `review.yaml` / `*.review.yaml` files with an explicit inspect-and-remove/archive message and leaves them untouched
- [ ] `rune adopt reseal --artifact <path>` updates reviewed sidecar digests after maintainer touch-ups and refuses pending inputs
- [ ] A finalize interrupted during sidecar writes keeps the temporary session, so rerunning finalize completes safely; `doctor --repair` remains a compatibility alias for verification

## The deck skill

`runes/meta/skills/adopt-artifact` (deck) drives this loop conversationally: it pulls `rune adopt status --json` through dynamic context injection, drafts one question per block, presents them four at a time through AskUserQuestion, and records your answers as verdicts. The CLI still enforces everything — the skill cannot skip a block, and finalize fails honestly when the edits do not match your decisions.
