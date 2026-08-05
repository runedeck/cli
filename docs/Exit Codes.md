# Exit Codes

The contract scripts and CI can rely on. Every command writes machine output
with `--json` on stdout and diagnostics on stderr.

- `0` — the command did what it says and found nothing wrong.
- `1` — findings or failure. Which one is command-specific but stable:
    - `validate`: schema errors (lint warnings alone stay 0)
    - `drift`: any drifted, missing, or unreadable entry in scope
    - `doctor --verify`: broken or orphaned managed files
    - `spec doctor`: a structurally broken change
    - `docs check`: broken links or orphans
    - `bench run`: any errored run, or nothing executed and nothing reused
    - `bench audit`: a fatal suite finding (short-token warnings alone stay 0)
    - everything else: the operation itself failed; the message on stderr
      names the cause
- `2` — usage errors (clap: unknown flags, missing arguments).

Mutating commands hold a per-target lock (`.rune.lock` in the deploy target)
for the duration of the write: a second `rune install`, `deploy`, or
`doctor --repair` against the same target fails fast with exit 1 instead of
interleaving. The lock names the holding pid and is removed when the process
exits.
