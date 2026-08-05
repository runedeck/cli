# Adopt Walkthrough

Manual test for the adoption review state machine (`rune adopt`, change: docs/changes/adopt-review, decision: CLI-0023). The flow: import an upstream artifact, give every block a verdict, let the CLI enforce the verdicts, seal the review record.

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

- [ ] `start` lands the artifact with adopt/v1 sidecars carrying `review: pending`, and a review record with every block `pending`
- [ ] `finalize` refuses while any block is pending, and lists the ids
- [ ] Cut a block, finalize WITHOUT editing the file: it refuses with "still appears verbatim"
- [ ] Delete a kept block's text, finalize: it refuses with "kept content missing"
- [ ] After a clean finalize: the record shows `status: reviewed`, your git identity as reviewer, the schema path + digest, and one entry per block; the sidecar flips to `review: reviewed` with the subject digest matching the file
- [ ] Text you added during editing shows up as `added` entries in the finalize output
- [ ] `rune adopt start` on the same artifact again: refused ("already passed review")
- [ ] `rune adopt abandon --yes` moves an in-flight adoption to `.trash/`, never deletes
- [ ] `rune provenance` and `rune validate` stay clean with review records present

## Hardening checks

- [ ] `rune install` over a module with a mid-review adoption skips it by name; the rest deploys; `rune install --strict` fails instead
- [ ] `rune release` and `rune copy` refuse outright while a review is open
- [ ] An adopt sidecar with its `review` field stripped does NOT deploy (fail closed, "adoption without review state")
- [ ] Blocks carrying injection-shaped content arrive flagged from `rune adopt next` (try a paragraph containing "ignore previous instructions"), and `verdict keep` on a flagged block demands `--note`
- [ ] The sealed record carries `decidedOn` and `transport: verdict-cli` per verdict, plus the `lint` and `segmenter` versions
- [ ] `rune adopt doctor` is clean after a good finalize; edit the reviewed file afterwards and it exits 1 naming the tampered subject; it warns on imports that never opened a review
- [ ] `rune adopt finalize --allow-new` records ceremony-authored files as `added` entries (default finalize refuses new files); such files stay sidecar-less and doctor reports them as authored, not missing
- [ ] After a finalize interrupted between sealing and sidecar flips, `rune adopt doctor --repair` re-syncs only sidecars whose file matches the sealed record's digest

## The deck skill

`runes/meta/skills/adopt-artifact` (deck) drives this loop conversationally: it pulls `rune adopt status --json` through dynamic context injection, drafts one question per block, presents them four at a time through AskUserQuestion, and records your answers as verdicts. The CLI still enforces everything — the skill cannot skip a block, and finalize fails honestly when the edits do not match your decisions.
