## 1. Implementation

- [x] 1.1 Deploy refusal: assembly fails closed on pending, stripped, and unreadable review states; SKILL.md governs its tree; `rune install --strict`, `rune release`, and `rune copy` refuse outright
- [x] 1.2 `decidedOn` timestamps on verdict writes; `transport` on entries (`verdict-cli`, `finalize` for added blocks)
- [x] 1.3 Injection lint (`lint/v1` pinned in the record): override/hijack phrasing, tool-call shapes, dynamic-injection lines, high-entropy runs, hidden unicode, external URLs in executable blocks; flags through `next`; keep-on-flagged requires a note
- [x] 1.4 `rune adopt doctor`: three-way digests, state coherence, completeness, orphaned imports, pacing warning; seal order fixed (record before sidecars); fully-cut deleted companions drop their sidecars
- [x] 1.5 Deck: `disallowed-tools: WebFetch, WebSearch` on adopt-artifact (kept through assembly via `keep_fields`); flagged-block question rule

## 2. Verification

- [x] 2.1 Tests: pending-skip and finalized-deploys on collection, strict inventory, timestamp/transport presence, flag detection with keep gating, doctor tamper case
- [x] 2.2 cargo fmt, clippy clean in scope, 1032 tests green; council (codex gpt-5.6-sol xhigh + grok) reviewed the design and its findings drove the implementation — a diff-level council pass is deferred until the shared working copy separates from the parallel bench work
- [x] 2.3 Walkthrough hardening checklist added (docs/walkthroughs/Adopt.md)

## 3. Follow-up

- [ ] 3.1 `rune adopt review` interactive TTY mode (own change; specced in adopt-hardening delta)
- [ ] 3.2 Doctor commit-layer checks: verify the sealed record against committed blobs and the introducing commit's signature (coordinated file+sidecar+record edits are invisible to path-level doctor)
- [ ] 3.3 Lint v2: markdown link-destination mismatch, homoglyph hosts, cross-block payloads
