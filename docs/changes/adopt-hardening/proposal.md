---
adr: "docs/decisions/CLI-0023 Adoption Review State Machine.md"
status: proposed
---
# Adopt Hardening

## Why

The review state machine (CLI-0023) enforces that verdicts exist before finalize, but three gaps remain between the review and the world. Unreviewed imports deploy: nothing in assemble or install consults the sidecar's pending state, so bytes that landed a minute ago become live skills in a harness directory. A faked review is invisible while a session runs: verdicts need timestamps and transport metadata so suspicious pacing is visible before finalize. Final reviewed sidecars are never re-checked: post-finalize edits, stalled sessions, and bypassed pipelines surface nowhere. Hostile upstream content also arrives unmarked, leaving detection of injection phrasing entirely to the reviewing model, the one party an injection targets.

## What Changes

- **Deploy refusal**: assembly excludes any source artifact whose adopt sidecar is not explicitly `review: reviewed` — pending, absent (a stripped or pre-state sidecar), and unparsable states all fail closed, naming each skipped artifact with a pointer to `rune adopt status`. A skill's `SKILL.md` state governs its whole tree (companions, scripts, deployed sidecars). The install completes otherwise; `rune install --strict` fails when anything was skipped, and `rune release` and `rune copy` refuse outright. First-party content (no adopt sidecar) is untouched.
- **Verdict timestamps**: `rune adopt verdict` stamps `decidedOn` (UTC RFC 3339) on the block entry, so the pending session carries the decision timeline until finalize.
- **Injection lint**: segmentation flags suspect blocks (`flags:` on the block entry): instruction-override phrasing, tool-invocation shapes, dynamic-injection lines in the adopted body, base64/high-entropy runs, URLs outside the upstream host. Flags ride through `next --json` so questions lead with them; they never block anything.
- **Doctor review pass**: `rune adopt doctor` reports pending external sessions, verifies reviewed sidecar subject digests against files, and diagnoses legacy review ledgers without treating them as authority.
- **Skill hardening (deck)**: `disallowed-tools: WebFetch, WebSearch` on adopt-artifact, removing the fetch channel while the review runs.
- **Interactive verdict channel**: specced here, built as its own change. `rune adopt review` presents blocks on the controlling TTY and records verdicts directly (`channel: tty` vs `channel: cli` on entries), removing the model from the decision datapath.

## Capabilities

- adopt-hardening (new; hardens the adopt-review state machine)

## Impact

- `src/cli/adopt/{segment,review}.rs` (flags, timestamps, channel field), assembly/deploy source walk, `src/cli/doctor.rs` (review pass).
- Deck: `runes/meta/skills/adopt-artifact/SKILL.md` frontmatter.
- Session additions are backward-compatible through serde defaults; existing reviewed sidecars remain deployable without ledgers.
