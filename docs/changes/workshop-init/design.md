# Workshop Init Design

## Approach

`rune init` absorbs the workshop scaffolder as a mode of the existing command rather than a new command: workshop mode is the default when the destination resolves under the configured workshop root (default `~/Agents`), and available anywhere via `--workshop`. Each integration is a separate idempotent step with `--dry-run`; the rejected alternative was one monolithic scaffold call, which couples VCS, vault, and deploy failures and cannot roll back.

## Structure

- Steps, each skippable and re-runnable: layout (private/public/assets), git init, jj colocate (only when jj is installed), entire hooks (only when entire is installed, and only with consent), commit/push hooks, `.rune` with `dirs:`, vault mount (explicit canonicalized association, never inside provider-managed trees).
- No automatic commit; init prints the suggested first commit instead.
- Satellites behind explicit flags: `--vault` (folder note), `--data` (data dir), `--remote` (private GitHub remote).
- `.rune` schema v2: `dirs:` entries `{path, role, access, required}`; committed paths are normalized relative paths resolved from the `.rune` file; absolute and `~` paths belong in gitignored `.rune.local`. The v1 reader stays; v2 is written only when `dirs:` is used. No recursive aggregation of nested `.rune` files.
- The VCS spine (`jj` + `entire`) outside workshop mode is opt-in via `--spine`, gated on tool presence.
- Rollback: init records created paths in its plan output; a failed run prints exactly what was created.

## Risks

- Partial failure mid-scaffold: idempotent steps plus the created-paths record make re-run and cleanup deterministic.
- Hook consent: hooks that push or capture sessions install only with an explicit yes (flag or interactive), regardless of workshop mode.
- Vault exposure: the mount is an association (path reference), not a copy, and lives outside `.claude` managed trees so prune never touches vault content.
