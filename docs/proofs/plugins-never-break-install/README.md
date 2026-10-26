---
type: proof
change: plugins-never-break-install
recorded: 2026-10-26
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
- scenario: plugins-never-break-install#manifest-listing/manifests-list
  kind: unproven
- scenario: plugins-never-break-install#post-install-event/subscribed-plugin-receives-the-event
  kind: unproven
- scenario: plugins-never-break-install#fault-isolation/failing-plugin-cannot-break-the-install
  kind: unproven
- scenario: plugins-never-break-install#executable-confinement/escaping-executable-is-rejected
  kind: unproven
---

# Proof: plugins-never-break-install

Pull request #55, "feat: add plugin manifests and a post-install event", merged this behavior. The fixture `tests/fixtures/plugins-never-break-install/` holds one home directory per scene, with its plugins under `.config/rune/plugins/`, and a consumer project whose `.rune` installs one rule from a local module. Each scene copies a home and the project into its own directory and points `HOME` at the copy. The fixture scripts carry no executable bit, so a scene that runs a plugin sets it after the copy. Every scene runs the real `rune` binary, and no scene stays unproven.

## plugins-never-break-install#manifest-listing/manifests-list

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/plugins-never-break-install/listing" home'
$ sh -c 'HOME="$PWD/home" rune plugin list'
 Plugins
   notifier post-install  Post a note when runes deploy
   syncer post-install  Mirror deployed runes to a second machine
```

## plugins-never-break-install#post-install-event/subscribed-plugin-receives-the-event

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/plugins-never-break-install/recorder" home && chmod +x home/.config/rune/plugins/recorder/run.sh'
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/plugins-never-break-install/project" project'
$ sh -c 'HOME="$PWD/home" rune install --source project --provider claude'

 ✓ claude
   rules 1
   ● Style

 ● 1 deployed
$ cat home/.config/rune/plugins/recorder/events.jsonl
{"deployed":1,"event":"post-install","providers":["claude"],"source":"[..]/project","target":null}
```

## plugins-never-break-install#fault-isolation/failing-plugin-cannot-break-the-install

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/plugins-never-break-install/broken" home && chmod +x home/.config/rune/plugins/broken/run.sh'
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/plugins-never-break-install/project" project'
$ sh -c 'HOME="$PWD/home" rune install --source project --provider claude'
warning: plugin 'broken' failed: exited exit status: 7

 ✓ claude
   rules 1
   ● Style

 ● 1 deployed
$ ls project/.claude/rules
Style.md
```

## plugins-never-break-install#executable-confinement/escaping-executable-is-rejected

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/plugins-never-break-install/escape" home'
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/plugins-never-break-install/project" project'
$ sh -c 'HOME="$PWD/home" rune plugin list'
 Plugins
   ⚡ [..]/home/.config/rune/plugins/escape: exec '../outside.sh' escapes the plugin directory; the plugin does not run
$ sh -c 'HOME="$PWD/home" rune install --source project --provider claude'
warning: [..]/escape: exec '../outside.sh' escapes the plugin directory; the plugin does not run
...
 ● 1 deployed
$ ls home/.config/rune/plugins
escape
outside.sh
```
