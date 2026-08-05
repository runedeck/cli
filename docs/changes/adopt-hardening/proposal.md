---
adr: "docs/decisions/CLI-0023 Adoption Review State Machine.md"
status: proposed
---
# Adopt Hardening

## Why

The review state machine (CLI-0023) enforces that verdicts exist before finalize, but three gaps remain between the review and the world. Unreviewed imports deploy: nothing in assemble or install consults the sidecar's pending state, so bytes that landed a minute ago become live skills in a harness directory. A faked review is invisible: the record proves verdicts were written, not when or through what channel, so a rubber-stamp loop is indistinguishable from a real session. A sealed record is never re-checked: post-seal edits, stalled sessions, and bypassed pipelines surface nowhere. Hostile upstream content also arrives unmarked, leaving detection of injection phrasing entirely to the reviewing model, the one party an injection targets.

## What Changes

- **Deploy refusal**: assembly excludes any source artifact whose adopt sidecar is not explicitly `review: reviewed` — pending, absent (a stripped or pre-state sidecar), and unparsable states all fail closed, naming each skipped artifact with a pointer to `rune adopt status`. A skill's `SKILL.md` state governs its whole tree (companions, scripts, deployed sidecars). The install completes otherwise; `rune install --strict` fails when anything was skipped, and `rune release` and `rune copy` refuse outright. First-party content (no adopt sidecar) is untouched.
- **Verdict timestamps**: `rune adopt verdict` stamps `decidedOn` (UTC RFC 3339) on the block entry, so the sealed record carries the decision timeline next to `startedOn`/`completedOn`.
- **Injection lint**: segmentation flags suspect blocks (`flags:` on the block entry): instruction-override phrasing, tool-invocation shapes, dynamic-injection lines in the adopted body, base64/high-entropy runs, URLs outside the upstream host. Flags ride through `next --json` so questions lead with them; they never block anything.
- **Doctor review pass**: `rune doctor` verifies sealed records three ways (record subject digest = file digest = sidecar digest), checks state coherence (record status vs sidecar `review`, adopt sidecars with no record, stalled pending sessions), completeness (no pending entries, notes on every adapt/cut), and warns on implausibly fast verdict pacing.
- **Skill hardening (deck)**: `disallowed-tools: WebFetch, WebSearch` on adopt-artifact, removing the fetch channel while the review runs.
- **Interactive verdict channel**: specced here, built as its own change. `rune adopt review` presents blocks on the controlling TTY and records verdicts directly (`channel: tty` vs `channel: cli` on entries), removing the model from the decision datapath.

## Capabilities

- adopt-hardening (new; hardens the adopt-review state machine)

## Impact

- `src/cli/adopt/{segment,review}.rs` (flags, timestamps, channel field), assembly/deploy source walk, `src/cli/doctor.rs` (review pass).
- Deck: `runes/meta/skills/adopt-artifact/SKILL.md` frontmatter.
- Record schema addition is backward-compatible (new optional fields); existing sealed records verify without them.
