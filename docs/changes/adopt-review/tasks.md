## 1. Implementation

- [x] 1.1 Deterministic markdown segmentation (`src/cli/adopt/segment.rs`, pulldown-cmark blocks, gap recovery, `segment/v1`)
- [x] 1.2 Review record types and read/write (`src/cli/adopt/review.rs`, in-toto statement with adoption-review predicate)
- [x] 1.3 `rune adopt` subcommand family: start, status, next, verdict, finalize, abandon; import alias retired
- [x] 1.4 `--kind agent|rule` placement and kebab-case naming in import
- [x] 1.5 Finalize consistency checks (whole-block multisets, kind-aware normalization) via the internal mdschema checker with trusted schema resolution
- [x] 1.6 Adopt sidecar pending/reviewed state; provenance scan skips review records; reviewed artifacts refuse re-import
- [x] 1.7 Deck: `.mdschema` per artifact kind encoding the skill-creator + dynamic-context standard (runes/meta)
- [x] 1.8 Deck: `adopt-artifact` 0.3.0 rewritten around the state machine and AskUserQuestion loop

## 2. Verification

- [x] 2.1 Tests: segmentation fixture, verdict lifecycle, finalize refusals (pending, cut-survives, keep-deleted), record shape, rule placement, kebab conversion
- [x] 2.2 cargo fmt, clippy clean in adopt scope; council review (codex gpt-5.6-sol xhigh + grok) applied to the design before implementation
- [x] 2.3 End-to-end dry run adopting a forge-core rule; walkthrough at docs/walkthroughs/Adopt.md

## 3. Deferred

- [ ] 3.1 DSSE envelope over the sealed statement (signed commit is the seal today; CLI-0023)
- [ ] 3.2 Span-anchored consistency (content multisets today; duplicate-blind, see CLI-0023 consequences)
- [ ] 3.3 `rune doctor` check for pending-review adoptions
- [ ] 3.4 Publish the predicate description at the recorded TypeURI (runedeck.github.io/attestation/adoption-review/v1)
- [ ] 3.5 Description-length warning (agentskills: ≤ 1024 chars, no angle brackets) in `rune validate`
- [ ] 3.6 Strict-spec export shape: anthropics/skills' validator allowlists exactly {name, description, license, allowed-tools, metadata, compatibility}; Claude Code extensions (version, argument-hint, context, agent, when_to_use, …) must move into `metadata` or be stripped when releasing for other harnesses
