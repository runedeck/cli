# Adopt Restructure Design

## Approach

One function decides which file a sidecar describes, and every reader calls it. The alternative, a per-reader rule with doctor trusting the holder and provenance trusting the name, is what let a hand move pass one check and fail the other. Writes concentrate in `rune repair`, so a doctor can run in any tree without side effects.

## Structure

- `src/cli/adopt/subject.rs`: `resolve_subject` and `SubjectFault`. Called from `review.rs` (doctor, reseal, repair) and `provenance/scan.rs`.
- `src/cli/adopt/review.rs`: `repair_sidecar_entries` is the one loop reseal and repair share. `trash_orphaned_sidecar` and `trash_stamp` give every prune the deploy convention. The replacement store is written by `write_replacement_store` before any sidecar.
- `src/cli/adopt/relocate.rs`: `rune move`. Plans every check before the first rename, pins the commit from jj when the repository is jj-backed and from git otherwise.
- `src/cli/repair.rs`: source pass, then deployment pass under the target lock. The deployment body moved here from `doctor.rs` unchanged.
- `src/cli/doctor.rs`: `inspect` only. `DoctorReport.repair_command` names the write path.
- `.provenance/` metadata types: `*.yaml` sidecars, `review.yaml` legacy ledgers, `replacements/` store, and `source-snapshot.json` deployment evidence. The scanners name each type. An unknown file is an error, never silence.

## Risks

- A false name disagreement from path aliasing (`/var` versus `/private/var`): both sides canonicalize before comparing. Covered by the subject tests on a temp root.
- Repair renaming a sidecar onto itself when the module root is not canonical: `trash_orphaned_sidecar` canonicalizes both paths and refuses a sidecar outside the root.
- A replacement whose blocks also exist as kept blocks: finalize counts kept copies before requiring the replacement's copies.
- `rune move` inside a jj workspace over a bare git store: git reports no work tree, so `jj workspace root` and `@-` supply the repository and commit.
