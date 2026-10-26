---
type: proof
change: dashboard-never-opens-empty
head: e8379ef1b4a847d532b59da0521296a882c2f759
head_change: owquyrkumrqwsxomynstmwsymuykorny
head_tree: b6da9100fa6232abfb60e238e8228271e4d223ee
recorded: 2026-10-26
transcript: e940355900cb86a8c4e8ccf2ea7850b7ce6b32bd8d77369509c8177ff49980de
scenes:
- scenario: dashboard-never-opens-empty#theme-derived-tui-palette/light-theme-selected
  kind: unproven
- scenario: dashboard-never-opens-empty#status-bar-context/provider-needs-repair
  kind: check
- scenario: dashboard-never-opens-empty#first-run-panel/tui-opens-on-an-unconfigured-root
  kind: unproven
- scenario: dashboard-never-opens-empty#first-run-panel/scan-fails-on-a-root-without-providers
  kind: check
- scenario: dashboard-never-opens-empty#help-overlay-close-keys/user-opens-help
  kind: check
---

# Proof: dashboard-never-opens-empty

Pull request #57, "feat: paint the TUI from the theme and route first runs into setup", merged this behavior. The scenes need no fixture: each one points `HOME` at its own empty scene directory, and the provider scene writes an empty `.claude/.manifest` there, which provider detection reads as an invalid deployment manifest. Every scene reads `rune tui --snapshot`, which prints one frame of the real dashboard as plain characters. The light-theme scene stays unproven because the snapshot carries no color. The unconfigured-root scene stays unproven because a defect on main keeps the first-run panel from ever appearing.

## dashboard-never-opens-empty#theme-derived-tui-palette/light-theme-selected

Unproven: the palette shows only in an interactive terminal, and `rune tui --snapshot` prints characters without their colors.

```console
```

## dashboard-never-opens-empty#status-bar-context/provider-needs-repair

```console
$ mkdir .claude
$ touch .claude/.manifest
$ sh -c 'HOME="$PWD" rune provider explain claude'
 claude
   config       bundled
   target       [..]/.claude
   state        needs repair
...
   deployment manifest  [..]/.claude/.manifest  invalid
...
$ sh -c 'HOME="$PWD" rune tui --snapshot --width 140 --height 4'
 rune  no deck  · claude ✗ [..]
...
```

## dashboard-never-opens-empty#first-run-panel/tui-opens-on-an-unconfigured-root

Unproven: a code defect keeps the panel out of the binary, because the scan fails on a root without provider directories, and on a root with one, `build_view` in `src/services/mod.rs` adds a `(no manifest)` placeholder module, so no scan ever reports no deck and no modules.

```console
```

## dashboard-never-opens-empty#first-run-panel/scan-fails-on-a-root-without-providers

```console
$ sh -c 'HOME="$PWD" rune tui --snapshot --width 100 --height 6'
 rune  no deck [..]
┌ Sections ─[..]
...
 NORMAL    error: no deployed provider directories found at ~ or .
```

## dashboard-never-opens-empty#help-overlay-close-keys/user-opens-help

```console
$ sh -c 'HOME="$PWD" rune tui --snapshot --width 100 --height 8 --keys "?"'
┌ Help · rune [..] · ? or Esc closes [..]
...
```
