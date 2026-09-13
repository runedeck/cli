# Command Map

How the overlapping command families relate, so the right tool is obvious.

## Running coding tools and skill scripts

- `launch` opens an interactive coding-tool session. It inherits terminal input and output, forwards native arguments, and permits terminal wrappers such as tmux.
- `run` executes a coding tool noninteractively. It reuses launch profiles and middleware, applies an explicit access mode, captures the final answer, and supervises the child process group. No timeout is applied unless requested.
- `exec` runs a script bundled with a skill through the runtime contract declared by that skill.

Use `launch` when a person will interact with the provider, `run` when automation needs a provider answer, and `exec` when the executable belongs to a skill.

## Deploying content

`install` is the user-facing verb: it assembles, deploys, and prunes in one
pass. The rest are its plumbing, useful when one stage is being debugged:

- `assemble` — transforms source into `build/` and stops. Builds into a
  staging tree and swaps only on success, so a failed run keeps the previous
  `build/` intact.
- `deploy` — copies an existing `build/` into provider targets and updates
  each target's `.manifest`.
- `copy` — verbatim copy with provenance but no transforms. For content that
  must land byte-identical.

Mutating commands hold a per-target lock. See [Exit Codes](Exit%20Codes.md).

## Checking health

The five check commands form a ladder. Run them in this order when something
looks wrong:

1. `validate` — is the source well-formed? (schemas, lint)
2. `status` — what does the deck intend? (specs, changes, deployments)
3. `drift` — does the deployment match the build? (diffs, missing files)
4. `doctor` — is the deployment intact? (`--verify` to fail CI. Doctor never
   writes, it names `rune repair` when something is repairable)
5. `provenance` — where did this deployed file come from? (forensics)

`bench doctor`, `spec doctor`, and `adopt doctor` are the same idea scoped to
their own subsystems. `adopt doctor` verifies pending external sessions and
reviewed adopt sidecar digests, and requires the sidecar holder and its
recorded `subject.name` to agree. Legacy review ledgers are migration
warnings, not final authority.

`repair` is the one command that writes to fix doctor findings: it trashes
orphan reviewed sidecars into `.trash/<stamp>/`, rewrites stale subject
names, restores missing managed files from a digest-matching build, and
quarantines deployment orphans. `--dry-run` prints the plan. A reviewed
subject whose bytes changed is not a repair. `adopt reseal --artifact <path>`
endorses the edit instead. `move <from> <to>` relocates one reviewed
artifact with its sidecars and records `transferredFrom: <artifact>@<commit>`.

## Bringing content in

- `import` — one-shot: fetch, align, write provenance sidecars, done.
- `adopt` — the reviewed path over the same import: a block-by-block external
  session (`start`, `next`, `verdict`, `finalize`) that seals final digests and
  concise review metadata into adopt/v1 sidecars, then removes the temporary
  block state. Content that will ship to other people goes through `adopt`.
  scratch experiments can use `import`.
