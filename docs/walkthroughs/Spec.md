# Spec Walkthrough

Review session for `rune spec`, the spec-driven-change lifecycle (OpenSpec dialect, rune root layout). Run in a scratch copy of the deck or any module.

## Lifecycle

```sh
rune spec propose add-widget --capability widgets --design
rune spec ls                      # draft  add-widget  0/N
rune spec show add-widget         # status · task progress · work order (+ design.md when present)
rune spec context add-widget      # machine-readable JSON for harness handoff
#   work happens; check [x] boxes in docs/changes/add-widget/tasks.md
rune spec doctor                  # complete → suggests archive
rune spec archive add-widget      # merges deltas into docs/specs/, removes the change
rune spec list --specs            # canonical capability specs with requirement counts
rune spec show widgets            # the merged capability spec
```

Expected at each step:

- `propose` scaffolds proposal.md, tasks.md, one delta per `--capability`, design.md with `--design`; a `## Capabilities` section lists the flags.
- `ls --sort progress` surfaces least-complete changes first; `name` is the default order.
- `doctor` exits 1 only on structural breakage (missing proposal or delta); completeness is a suggestion, not an error.
- `archive --abandon -y` discards without merging, scripted.
- Template overrides: a `templates/spec/*.md` file at the source root beats the embedded copy; upstream updates are a file copy.

## Root layout

Native root is `docs/changes` + `docs/specs`. The openspec-interop change (see docs/changes/openspec-interop/) adds a compat mode over an `openspec/` root plus export/import converters; until it lands, OpenSpec-rooted repos are out of scope.

## Review checklist

- [ ] propose → archive round trip leaves `spec doctor` healthy
- [ ] multi-capability propose dedupes repeated flags and scaffolds one delta each
- [ ] design.md appears in both `show` and `context` output
- [ ] `show` on a name that is both change and spec errors listing both forms
- [ ] template override wins over the embedded template
