# Command Map

How the overlapping command families relate, so the right tool is obvious.

## Running coding tools and skill scripts

- `launch` opens an interactive coding-tool session. It inherits terminal input and output, forwards native arguments, and permits terminal wrappers such as tmux.
- `run` executes a coding tool noninteractively. It reuses launch profiles and middleware, applies an explicit provider sandbox mode, captures the final answer, and supervises the child process group. No timeout is applied unless requested.
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
- `copy` — verbatim copy with provenance but no transforms; for content that
  must land byte-identical.

Mutating commands hold a per-target lock; see [Exit Codes](Exit%20Codes.md).

## Checking health

The five check commands form a ladder; run them in this order when something
looks wrong:

1. `validate` — is the source well-formed? (schemas, lint)
2. `status` — what does the deck intend? (specs, changes, deployments)
3. `drift` — does the deployment match the build? (diffs, missing files)
4. `doctor` — can the deployment be repaired? (`--verify` to fail CI,
   `--repair` to restore from build and quarantine orphans)
5. `provenance` — where did this deployed file come from? (forensics)

`bench doctor` and `spec doctor` are the same idea scoped to their own
subsystems.

## Bringing content in

- `import` — one-shot: fetch, align, write provenance sidecars, done.
- `adopt` — the reviewed path over the same import: a block-by-block session
  (`start`, `next`, `verdict`, `finalize`) that seals maintainer verdicts
  into a review record. Content that will ship to other people goes through
  `adopt`; scratch experiments can use `import`.
