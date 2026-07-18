# Adopt Split Design

## Approach

Today's `rune adopt` is a byte-copy with provenance sidecars; the name promises more than it does. Split: `rune import` becomes the copy, `rune adopt` becomes the harness-driven adoption process. The rejected alternative was a `--raw` flag on one command, which hides the trust boundary between copying foreign bytes and executing a process over them.

## Structure

- `rune import <path> --module <m> --name <n>`: exactly today's adopt (alignment, byte-for-byte companions, one provenance sidecar per file, `--dry-run`). `adopt` remains a deprecated alias for one release, printing the rename note.
- `rune adopt <path>`: runs `import` first, then launches the configured harness with the adoption skill: review the artifact, rewrite to deck conventions (naming, description triggers, structure), validate, and report. The process is formalized as a deck skill so it evolves without recompiling.
- Trust boundary: adopt never executes imported content; the harness reads and rewrites it. Import marks the artifact `adopted: pending-review` in provenance until adopt completes.

## Risks

- Muscle memory and docs referencing `adopt` for the copy: the deprecated alias plus rename note covers one release; completions list both with the note.
- Harness variance: the adoption skill is the contract; rune only launches and checks the exit state.
