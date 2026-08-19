# Manual Testing

A hands-on walkthrough of the rune + deck system, one section per subcommand, grouped the way `rune --help` groups them. Every step names its expected result.

## Setup

`DECK` is the content repo; steps assume `~/.cargo/bin` on PATH.

```sh
export DECK=~/Developer/runedeck/deck
export PATH="$HOME/.cargo/bin:$PATH"
cd ~/Developer/runedeck/cli
cargo install --path .
rune --version        # rune 0.5.0 (<commit>) built <timestamp> — the hash tracks HEAD
rune --help           # runic wordmark, then groups: Flow, Spec, Deck, Plumbing
```

Expected:

- on a TTY the wordmark renders in color (cyan sigil, dim tagline); piped output is plain text
- global flags on every command: `--json` (machine output, no styling) and `--no-color` (plain text on a TTY)
- the exit-code and locking contract is in [Exit Codes](Exit%20Codes.md); how the overlapping command families relate is in [Command Map](Command%20Map.md)

Two throwaway sandboxes serve the whole walkthrough: `T` is the deployed consumer several sections reuse, `RUNE_TARGETS` is the targets root the init section scaffolds into.

```sh
export RUNE_TARGETS="$(mktemp -d)"
T=$(mktemp -d) && cd "$T" && rune target . && rune add BuildSkill,ArtifactLength && rune install
```

The deck's casts are being rewired to the `runes/core` layout during the adoption pass; until they land, cast selection (`--cast …`) fails with `cast 'base' rune pattern 'development/rules/**' matches no rune`, which is itself the expected fail-loud behavior for a stale cast. Fixtures below select runes by id instead.

## Flow

### rune setup

```sh
rune setup --defaults          # reports deck + target state without prompting
```

Expected: no prompts with `--defaults`; `setup --json` emits pure JSON.

### rune config

```sh
rune config path               # ~/.config/rune/config.yaml
rune config get deck           # raw value, exit 0; exit 1 when unset
rune config set owner tester && rune config get owner && rune config unset owner
#   owner = default owner segment for init slugs: `rune init demo` scaffolds
#   under <targets>/<owner>/demo; an explicit `rune init acme/demo` overrides it
rune config set bench ~/Developer/N4M3Z/bench   # appends to the bench workspace list
```

Expected:

- `bench` is a list: each `config set bench <path>` appends (dedup), `config get bench` prints the list, `config unset bench` clears it
- an unknown key errors naming the supported set

### rune completion

```sh
rune completion print zsh | head -3   # a real #compdef script
rune completion print nushell | head -3
rune completion install               # writes the standard location and clears the shell's own compdump
zcompinit                             # reload completions in this shell, no restart needed
```

Expected:

- `completion print zsh | head -3` prints `#compdef rune` and exits 0 (a closed pipe is not an error)
- `completion install` names the cleared completion cache (unrelated `~/.zcompdump*` files are deliberately preserved)
- `zcompinit` (the dotfiles function running `autoload -Uz compinit && compinit`) picks the new completions up in the running shell
- `completion print` also serves bash and fish

### rune init

```sh
export RUNE_TARGETS="$(mktemp -d)"
rune init demo --with shell,tool --dry-run   # plan only, destination untouched
rune init demo --with shell,tool --brief "Manual init target"
cd "$RUNE_TARGETS/demo"
test -x bin/demo && test -x .githooks/pre-commit && echo hooks-ok
git config --get core.hooksPath       # .githooks
ls -d private public assets .jj       # the workshop layout + jj colocation
git rev-parse --verify HEAD           # "fatal: Needed a single revision" — correct:
#   workshop init never auto-commits; the first commit stays yours
```

Expected:

- init lists the applied layers: `base`, `shell`, `tool` and writes `answers.yaml`
- under the targets root init runs in workshop mode: the private/public/assets layout lands, jj colocates when installed, and nothing is committed (`--workshop` forces the mode elsewhere; `--spine` gives a plain project the jj colocation; outside the targets root a plain init still commits the scaffold)
- `--skeleton <DIR>` overrides the skeleton root; `--bind` makes the fresh project the active target in one step; `rune init --module <DIR>` scaffolds a deck module instead
- the composed `.gitignore` carries the base and selected-template entries
- `rune validate` in the scaffolded project runs consumer checks, so the pre-commit hook passes
- with no configured skeleton root, init extracts the built-in layers to a per-version cache, so scaffolding works from a bare brew install

### rune target

```sh
rune target .                          # binds this working repo as the target
rune target --list                     # recent targets with the active binding marked
rune target -                          # restores the previous binding
rune target --unbind                   # clears the binding
```

Expected: `rune target <owner>/<name> --clone` fetches a missing target from GitHub into the targets root.

### rune add

```sh
cd "$RUNE_TARGETS/demo" && rune target .
rune add BuildSkill                    # the configured deck supplies the source
rune add core/skills/BuildSkill        # fully qualified <domain>/<kind>/<Name> works too
cd "$(mktemp -d)" && rune add BuildSkill          # the target-redirect note
```

Expected:

- the last command prompts `no .rune here; stage into the bound target at …? [Y/n]`
- answering n (or EOF, or any non-interactive run) cancels with `staging cancelled` and writes nothing
- nothing ever lands outside the current directory without an explicit yes

### rune skill

```sh
rune skill add BuildSkill --source "$DECK"        # bare name → core/skills/BuildSkill
rune skill add nonexistent            # fatal: no skills rune named 'nonexistent'
rune skill show | head -5             # frontmatter: name rune, current version
rune skill install --dir "$(mktemp -d)"   # installs under <dir>/.claude/skills/rune/SKILL.md
```

Expected: `skill install` refuses a symlinked destination and defaults to `~/.claude/skills` when `--dir` is omitted.

### rune agent · rule · hook

```sh
rune rule add ArtifactLength          # → core/rules/ArtifactLength
rune agent add TheOpponent            # fatal: no agents rune named 'TheOpponent' — none adopted yet
```

Expected:

- each kind command echoes the fully qualified id it staged
- a bare name present in two domains errors listing both; `<domain>/<name>` disambiguates
- the bare noun (`rune agent`) lists that kind
- agents and hooks join these fixtures as the adoption pass lands them in the deck

### rune context

```sh
rune context                          # root (consumer) · selection · providers · next: rune install
```

### rune tui

Covered in depth by the dedicated TUI walkthrough (docs/walkthroughs/). Quick pass:

```sh
cd "$DECK" && rune tui
rune tui --edit                       # checkbox editor with the selection ready
```

Miller-column navigation, `/` in-pane filter, `!` problems-only, History batched loading. Code tab: `12j`, `5G`, `gg`, `zz`, `]]`/`[[`, `/` + `n/N`, `V` + `c` range comments; Enter saves, Esc (twice when dirty) cancels; wheel scroll moves only the viewport.

### rune dashboard

```sh
rune dashboard --source "$DECK"        # read-only web dashboard on localhost
```

### rune install

In the deployed consumer fixture:

```sh
ls .claude/rules                      # ArtifactLength.md (rules stay PascalCase)
ls .claude/skills/rune/skills         # BuildSkill (the rune plugin root)
cat .claude/skills/rune/.claude-plugin/plugin.json   # the namespace source: name rune
```

All four providers, wider selection:

```sh
T=$(mktemp -d) && cd "$T" && rune target .
rune add --source "$DECK" BuildSkill,AdoptArtifact,ArtifactLength,SentenceCase >/dev/null
rune install                          # one ● line per artifact, per provider
rune install --force --verbose        # per-file listing, unchanged skips included
for p in .claude .codex .gemini .opencode; do echo "$p: $(find $p -type f | wc -l)"; done
```

Expected:

- default output per provider: a kind-count line (`agents 3  rules 1  skills 28`) and one `●` line per artifact (`● BuildSkill`); user-modified skips and prunes still print
- `--verbose` restores the per-file `●` listing and every skip reason
- every provider directory carries the same deployment, with .claude two files ahead of the others (the plugin manifest and the plugin root's own .manifest; selections that include hooks add a merged hooks.json too)
- Claude Code loads the tree as the rune@skills-dir plugin, so skills invoke as /rune:\<name\>

Pinned git install (the remote-consumer path): a `.rune` whose source is `git: https://…` plus a full `ref:` SHA materializes the deployment from the pinned commit. Release binaries accept only `https://` git URLs; the `file://` form used by the integration tests exists solely behind the `test-file-urls` cargo feature (`cargo run --features test-file-urls` with `RUNE_GIT_ALLOW_FILE_URLS=1`), so this step waits for the deck's public remote.

Variants worth one pass each: `--provider claude` (repeatable provider filter), `--only <prefix>` (source-relative prefix, implies `--no-prune`), `--dry-run` (show what pruning would move), `--no-prune` (keep stale files), `--force` (overwrite user-modified deployed files), `--allow-stale` (skip the behind-origin freshness stop), `--model <id>` (model-qualifier variants).

Rule wiring on a home-scope install (`--target ~`, module-root source): only Claude reads a rules directory, so install also maintains a marker-delimited generated block in `~/.codex/AGENTS.md` and `~/.gemini/GEMINI.md` (superseding forge-provision's `harness-rules` block) and ensures the rules glob in the `instructions` array of `~/.config/opencode/opencode.json`:

```sh
grep -c "rune-rules:begin" ~/.codex/AGENTS.md     # 1 — exactly one generated region
rune install --source "$DECK/runes/core" --target ~ && rune install --source "$DECK/runes/core" --target ~
#   second install leaves AGENTS.md byte-identical (idempotent)
ls ~/.codex/rules 2>/dev/null                     # markdown rules are NOT deployed here:
#   codex reserves this directory for .rules command policies and never reads
#   deployed markdown; its rules arrive only through the AGENTS.md block
```

Expected:

- content outside the markers is never touched
- headless Antigravity currently loads no file context at all, so the GEMINI.md block serves gemini-cli and interactive Antigravity only
- project-scope installs (`--target <repo>`) never edit a repo's instruction files and keep deploying rules directories as before

### rune review

Inspects the review comments the TUI Code tab persists to `.rune-comments.yaml` (`V` + `c` writes a range comment; Enter saves). With no saved comments both commands print nothing and exit 0.

```sh
cd "$DECK" && rune tui                # Code tab: V, move, c, type a comment, Enter, q
rune review list --target "$DECK"     # one line per saved comment: file:line-range + text
rune review export --target "$DECK" --format markdown   # comments with their source-line context
```

## Spec

The dedicated [Spec walkthrough](walkthroughs/Spec.md) explains the lifecycle. These sections exercise the Rust implementation against native, OpenSpec, and custom roots. The artifact dialect is pinned to [OpenSpec v1.6.0][OPENSPEC-160]. Normal `rune spec` commands do not invoke the upstream OpenSpec CLI.

### Native `docs/` root

Prerequisites and setup:

```sh
NATIVE_ROOT=$(mktemp -d)
rune spec propose add-widget --capability widgets --design --source "$NATIVE_ROOT"
python3 -c 'from pathlib import Path; path = Path(__import__("sys").argv[1]); path.write_text(path.read_text().replace("- [ ]", "- [x]"))' "$NATIVE_ROOT/docs/changes/add-widget/tasks.md"
```

Commands:

```sh
rune spec list --source "$NATIVE_ROOT"
rune spec show add-widget --source "$NATIVE_ROOT"
rune spec context add-widget --source "$NATIVE_ROOT"
rune spec validate add-widget --source "$NATIVE_ROOT"
rune spec archive add-widget --source "$NATIVE_ROOT"
rune spec validate widgets --source "$NATIVE_ROOT"
rune spec list --specs --source "$NATIVE_ROOT"
```

Expected results:

- `propose` creates the proposal, checklist, design, and `docs/changes/add-widget/specs/widgets/spec.md`.
- Targeted validation accepts both the active change and the canonical `widgets` capability.
- Archive creates `docs/specs/widgets/spec.md` and moves the change under `docs/changes/archive/<date>-add-widget`.

Recoverable cleanup:

```sh
trash "$NATIVE_ROOT"
```

### Direct `openspec/` root and advisory doctor check

Prerequisites and setup:

```sh
OPENSPEC_ROOT=$(mktemp -d)
ADVISORY_BIN=$(mktemp -d)
mkdir -p "$OPENSPEC_ROOT/openspec/specs/widgets"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("# Widgets Specification\n\n## Purpose\n\nDescribe widgets.\n\n## Requirements\n\n### Requirement: Widget lookup\n\nThe system SHALL return a widget.\n\n#### Scenario: Widget exists\n\n- **WHEN** a widget exists\n- **THEN** the widget is returned\n")' "$OPENSPEC_ROOT/openspec/specs/widgets/spec.md"
python3 -c 'from pathlib import Path; path = Path(__import__("sys").argv[1]); path.write_text("#!/bin/sh\nprintf fixture-validation-failed >&2\nexit 1\n"); path.chmod(0o755)' "$ADVISORY_BIN/openspec"
```

Commands:

```sh
rune spec list --source "$OPENSPEC_ROOT" </dev/null
(cd "$OPENSPEC_ROOT" && rune spec list)     # choose 1 to keep openspec/
rune spec validate widgets --source "$OPENSPEC_ROOT"
ADVISORY_OUTPUT=$(PATH="$ADVISORY_BIN:$PATH" rune spec doctor --source "$OPENSPEC_ROOT")
ADVISORY_STATUS=$?
grep -F "warning: openspec validate: the upstream OpenSpec CLI reports issues (advisory): fixture-validation-failed" <<<"$ADVISORY_OUTPUT"
test "$ADVISORY_STATUS" -eq 0
rune spec doctor --source "$OPENSPEC_ROOT"
```

Expected results:

- The non-interactive list prints one advisory note, writes no config, and reads `openspec/`.
- The interactive choice records `spec.root: openspec` in the repository `config.yaml` and does not repeat.
- Doctor attempts `openspec validate --all --no-interactive` only when the executable is available. The fixture failure produces one bounded advisory warning and leaves the exit status at 0 because rune reports no structural error. The final command uses the installed upstream executable when available and otherwise skips the advisory without a finding.

Recoverable cleanup:

```sh
trash "$OPENSPEC_ROOT"
trash "$ADVISORY_BIN"
```

### Custom repository-relative root

Prerequisites and setup:

```sh
CUSTOM_ROOT=$(mktemp -d)
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("spec:\n    root: artifacts/specifications\n")' "$CUSTOM_ROOT/config.yaml"
```

Commands:

```sh
rune spec propose custom-widget --capability widgets --source "$CUSTOM_ROOT"
rune spec validate custom-widget --source "$CUSTOM_ROOT"
test -f "$CUSTOM_ROOT/artifacts/specifications/changes/custom-widget/specs/widgets/spec.md"
```

Expected results:

- Every lifecycle command uses `artifacts/specifications/changes` and `artifacts/specifications/specs`.
- A root containing `..`, an absolute path, or a symlinked boundary fails before mutation.

Recoverable cleanup:

```sh
trash "$CUSTOM_ROOT"
```

### Delta operations and application order

Prerequisites and setup:

```sh
DELTA_ROOT=$(mktemp -d)
mkdir -p "$DELTA_ROOT/docs/specs/search" "$DELTA_ROOT/docs/changes/update-search/specs/search"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("# Search Specification\n\n## Purpose\n\nDescribe search.\n\n## Requirements\n\n### Requirement: Legacy lookup\n\nThe system SHALL return a document.\n\n#### Scenario: Document exists\n\n- **WHEN** a document exists\n- **THEN** the document is returned\n\n### Requirement: Obsolete filter\n\nThe system SHALL expose an obsolete filter.\n\n#### Scenario: Filter requested\n\n- **WHEN** the filter is requested\n- **THEN** the filter is returned\n")' "$DELTA_ROOT/docs/specs/search/spec.md"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("## ADDED Requirements\n\n### Requirement: Owner filter\n\nThe system SHALL filter documents by owner.\n\n#### Scenario: Owner selected\n\n- **WHEN** an owner is selected\n- **THEN** only matching documents are returned\n\n## MODIFIED Requirements\n\n### Requirement: Current lookup\n\nThe system SHALL return a document with its path.\n\n#### Scenario: Document exists\n\n- **WHEN** a document exists\n- **THEN** the document and path are returned\n\n## RENAMED Requirements\n\n- FROM: `### Requirement: Legacy lookup`\n- TO: `### Requirement: Current lookup`\n\n## REMOVED Requirements\n\n- `### Requirement: Obsolete filter`\n")' "$DELTA_ROOT/docs/changes/update-search/specs/search/spec.md"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("---\nstatus: proposed\n---\n# Update Search\n\n## Why\n\nKeep search behavior explicit.\n\n## What Changes\n\n- Update lookup and filtering.\n\n## Capabilities\n\n- search (modified)\n\n## Impact\n\n- Search behavior\n")' "$DELTA_ROOT/docs/changes/update-search/proposal.md"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("## Implementation\n\n- [x] Apply the search delta\n")' "$DELTA_ROOT/docs/changes/update-search/tasks.md"
```

Commands:

```sh
rune spec validate update-search --source "$DELTA_ROOT"
rune spec archive update-search --source "$DELTA_ROOT"
grep -n "Requirement:" "$DELTA_ROOT/docs/specs/search/spec.md"
```

Expected results:

- Archive applies `RENAMED`, `REMOVED`, `MODIFIED`, then `ADDED`, regardless of section order in the delta.
- `Legacy lookup` becomes `Current lookup` before the modification resolves.
- `Obsolete filter` is absent and `Owner filter` is appended.

Recoverable cleanup:

```sh
trash "$DELTA_ROOT"
```

### Nested capabilities

Prerequisites and setup:

```sh
NESTED_ROOT=$(mktemp -d)
rune spec propose add-card --capability payments/card --source "$NESTED_ROOT"
python3 -c 'from pathlib import Path; path = Path(__import__("sys").argv[1]); path.write_text(path.read_text().replace("- [ ]", "- [x]"))' "$NESTED_ROOT/docs/changes/add-card/tasks.md"
```

Commands:

```sh
rune spec validate add-card --source "$NESTED_ROOT"
rune spec context add-card --source "$NESTED_ROOT"
rune spec archive add-card --source "$NESTED_ROOT"
rune spec show payments/card --source "$NESTED_ROOT"
rune spec validate payments/c --source "$NESTED_ROOT"
```

Expected results:

- Context names the capability `payments/card`.
- Archive writes `docs/specs/payments/card/spec.md`.
- Full names and unambiguous nested prefixes resolve for show and validation.

Recoverable cleanup:

```sh
trash "$NESTED_ROOT"
```

### Import, export, ownership, and unknown artifact preservation

Prerequisites and setup:

```sh
ROUNDTRIP_ROOT=$(mktemp -d)
mkdir -p "$ROUNDTRIP_ROOT/openspec/specs/widgets" "$ROUNDTRIP_ROOT/openspec/schemas/custom" "$ROUNDTRIP_ROOT/expected"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("spec:\n    root: docs\n")' "$ROUNDTRIP_ROOT/config.yaml"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("# Widgets Specification\n\n## Purpose\n\nDescribe widgets.\n\n## Requirements\n\n### Requirement: Widget lookup\n\nThe system SHALL return a widget.\n\n#### Scenario: Widget exists\n\n- **WHEN** a widget exists\n- **THEN** the widget is returned\n")' "$ROUNDTRIP_ROOT/openspec/specs/widgets/spec.md"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("# Example project\n")' "$ROUNDTRIP_ROOT/openspec/project.md"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("schema: custom\n")' "$ROUNDTRIP_ROOT/openspec/schemas/custom/schema.yaml"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_bytes(bytes([0, 1, 127, 255]))' "$ROUNDTRIP_ROOT/openspec/unknown.bin"
command cp "$ROUNDTRIP_ROOT/openspec/project.md" "$ROUNDTRIP_ROOT/expected/project.md"
command cp "$ROUNDTRIP_ROOT/openspec/schemas/custom/schema.yaml" "$ROUNDTRIP_ROOT/expected/schema.yaml"
command cp "$ROUNDTRIP_ROOT/openspec/unknown.bin" "$ROUNDTRIP_ROOT/expected/unknown.bin"
```

Commands:

```sh
rune spec import --openspec --source "$ROUNDTRIP_ROOT"
test -f "$ROUNDTRIP_ROOT/docs/.interop/openspec/manifest.yaml"
test -f "$ROUNDTRIP_ROOT/docs/.interop/openspec/files/project.md"
OPAQUE_JSON=$(rune spec validate --json --source "$ROUNDTRIP_ROOT")
python3 -c 'import json, sys; items = json.loads(sys.argv[1]); assert any(item["code"] == "opaque-artifact" for item in items)' "$OPAQUE_JSON"
rune spec export --openspec --source "$ROUNDTRIP_ROOT"
cmp "$ROUNDTRIP_ROOT/expected/project.md" "$ROUNDTRIP_ROOT/openspec/project.md"
cmp "$ROUNDTRIP_ROOT/expected/schema.yaml" "$ROUNDTRIP_ROOT/openspec/schemas/custom/schema.yaml"
cmp "$ROUNDTRIP_ROOT/expected/unknown.bin" "$ROUNDTRIP_ROOT/openspec/unknown.bin"
```

Expected results:

- Import moves change and specification artifacts into `docs/` and stores every other regular file under `.interop/openspec/files/` without interpreting its body.
- `.interop/openspec/manifest.yaml` records the original path, classification, and SHA-256 digest for each owned artifact.
- Validation reports opaque artifacts as warnings.
- Export verifies ownership and restores the original paths and bytes. Only one live tree remains after each conversion.

Recoverable cleanup:

```sh
trash "$ROUNDTRIP_ROOT"
```

### Interrupted archive and conversion recovery

Failure injection is test-only. `rune spec` has no public flag for interrupting a transaction, so these focused Cargo tests exercise the recorded recovery phases directly.

Prerequisites and setup:

```sh
RECOVERY_LOGS=$(mktemp -d)
cd ~/Developer/runedeck/cli
```

Commands:

```sh
cargo test --manifest-path rune-docs/Cargo.toml --features lifecycle archive_moved_recovery_finishes_the_commit | tee "$RECOVERY_LOGS/archive.log"
cargo test --manifest-path rune-docs/Cargo.toml --features lifecycle import_recovery_finishes_source_removal_through_shared_transaction | tee "$RECOVERY_LOGS/import.log"
cargo test --manifest-path rune-docs/Cargo.toml --features lifecycle export_recovery_finishes_source_removal_through_shared_transaction | tee "$RECOVERY_LOGS/export.log"
```

Expected results:

- The archive test resumes from a recorded archive move and completes canonical and archive state.
- The conversion tests resume after a source-removal interruption and finish with one live tree, verified destinations, and cleared transaction state.
- A failure leaves `.rune-transaction/` available for inspection. The next archive, import, or export command performs recovery before new work.

Recoverable cleanup:

```sh
trash "$RECOVERY_LOGS"
```

### Idempotent retries and no-op modification times

Prerequisites and setup:

```sh
RETRY_ROOT=$(mktemp -d)
rune spec propose retry-widget --capability widgets --source "$RETRY_ROOT"
python3 -c 'from pathlib import Path; path = Path(__import__("sys").argv[1]); path.write_text(path.read_text().replace("- [ ]", "- [x]"))' "$RETRY_ROOT/docs/changes/retry-widget/tasks.md"
rune spec archive retry-widget --source "$RETRY_ROOT"
ARCHIVE_PATH=$(find "$RETRY_ROOT/docs/changes/archive" -type d -name '*-retry-widget')
CANONICAL_MTIME=$(stat -f '%m' "$RETRY_ROOT/docs/specs/widgets/spec.md")
ARCHIVE_MTIME=$(stat -f '%m' "$ARCHIVE_PATH")
```

Commands:

```sh
sleep 1
rune spec archive retry-widget --source "$RETRY_ROOT"
test "$CANONICAL_MTIME" = "$(stat -f '%m' "$RETRY_ROOT/docs/specs/widgets/spec.md")"
test "$ARCHIVE_MTIME" = "$(stat -f '%m' "$ARCHIVE_PATH")"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("spec:\n    root: docs\n")' "$RETRY_ROOT/config.yaml"
mkdir -p "$RETRY_ROOT/openspec/specs/other"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("# Other Specification\n\n## Purpose\n\nDescribe another capability.\n\n## Requirements\n\n### Requirement: Other behavior\n\nThe system SHALL provide another behavior.\n\n#### Scenario: Behavior requested\n\n- **WHEN** the behavior is requested\n- **THEN** the behavior is provided\n")' "$RETRY_ROOT/openspec/specs/other/spec.md"
rune spec import --openspec --source "$RETRY_ROOT"
IMPORT_MTIME=$(stat -f '%m' "$RETRY_ROOT/docs/.interop/openspec/manifest.yaml")
sleep 1
rune spec import --openspec --source "$RETRY_ROOT"
test "$IMPORT_MTIME" = "$(stat -f '%m' "$RETRY_ROOT/docs/.interop/openspec/manifest.yaml")"
```

Expected results:

- Repeating a completed archive exits successfully, reports the existing archive, and preserves canonical and archive modification times.
- Repeating an import with a verified ownership manifest performs no writes and preserves the manifest modification time.

Recoverable cleanup:

```sh
trash "$RETRY_ROOT"
```

### JSON validation diagnostics

Prerequisites and setup:

```sh
JSON_ROOT=$(mktemp -d)
mkdir -p "$JSON_ROOT/docs/specs"
python3 -c 'from pathlib import Path; Path(__import__("sys").argv[1]).write_text("# Invalid Root Specification\n\n## Purpose\n\nThis file has no capability directory.\n\n## Requirements\n")' "$JSON_ROOT/docs/specs/spec.md"
```

Commands:

```sh
set +e
DIAGNOSTICS=$(rune spec validate --json --source "$JSON_ROOT")
VALIDATE_STATUS=$?
set -e
python3 -c 'import json, sys; item = json.loads(sys.argv[1])[0]; required = {"code", "severity", "path", "line", "column", "message", "operation", "capability", "change"}; assert required <= item.keys(); assert item["line"] is None and item["column"] is None and item["operation"] is None and item["capability"] is None and item["change"] is None; print(json.dumps(item, sort_keys=True))' "$DIAGNOSTICS"
test "$VALIDATE_STATUS" -eq 1
```

Expected results:

- Validation exits unsuccessfully because the root-level `spec.md` has no capability identifier.
- The JSON diagnostic includes every stable field. Missing line, column, operation, capability, and change context appear as explicit `null` values.

Recoverable cleanup:

```sh
trash "$JSON_ROOT"
```

[OPENSPEC-160]: https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0 "OpenSpec v1.6.0 release"

## Deck

### rune status

```sh
cd "$DECK" && rune status              # deck · specs · changes · deployments dashboard
```

### rune doctor

In the deployed consumer fixture:

```sh
rune doctor --target .                # summary of managed-file states, left untouched
rune doctor --target . --verify       # exit nonzero while broken/orphaned files exist
rune doctor --target . --repair       # restores missing managed files, quarantines orphans
```

On a wired home target, doctor also checks the rule-wiring block: delete the generated region from `~/.codex/AGENTS.md` and `rune doctor --target ~/.codex --verify` exits 1 with a `(rule wiring)` finding.

### rune validate

```sh
cd "$DECK" && rune validate          # fast (~0.2s), aggregate over all domains, no errors
rune validate --scan                 # + gitleaks and semgrep, the commit/push-hook mode
cd "$(mktemp -d)" && rune validate   # refuses: not a rune source; --force overrides
```

Expected:

- deck source names are PascalCase (kebab-case also validates; snake_case fails with a pattern error); providers that want kebab filenames get them at assembly time
- validate never walks a directory without `deck.yaml`/`module.yaml`/`.rune` unless forced; a root carrying both `module.yaml` and `.rune` gets both check sets
- in a scaffolded consumer, validate runs the consumer checks (`.rune` parses, per-provider manifests), so the pre-commit hook passes

Lint warnings (non-blocking): validate also warns when a skill description lacks trigger phrasing ("use when", "invoke", …), when the name contains `claude`/`anthropic` or diverges from its directory, when name/description exceed the 64/1024 agentskills.io limits, when angle brackets are unmatched, or when the body is under 50 chars.

A guardrail worth seeing fail:

```sh
cd "$DECK"
sed -i '' 's/^schema: 1/schema: 2/' deck.yaml
rune validate         # hard error naming found schema 2 vs supported 1
sed -i '' 's/^schema: 2/schema: 1/' deck.yaml
```

### rune drift

In the deployed consumer fixture:

```sh
rune drift --target .                 # clean, exit 0
echo tamper >> .claude/rules/ArtifactLength.md
rune drift --target .                 # flags ArtifactLength.md as modified
rune drift --target . --all           # also lists Identical files (hidden by default)
rune install --force                  # user modifications need --force; doctor --repair leaves them
rune drift --target .                 # clean again, exit 0
```

Variants: `--upstream <DIR>` (two source trees by name), `--ignore body` (ignore body drift), `--source <DIR>` (source vs build).

### rune provenance

```sh
rune provenance --target .claude/rules/ArtifactLength.md   # per-file provenance chain
```

### rune clean

```sh
rune clean --target .                 # removes stale installed files no source still ships
```

### rune release

In a deployed consumer:

```sh
rune release --format cowork           # dist/rune-cowork-plugin.zip, limits enforced
```

### rune import

```sh
M=$(mktemp -d) && rune init --module "$M/scratch"
rune import <path-to-skill-dir> --module "$M/scratch" --name ExampleImported --dry-run
rune import <path-to-skill-dir> --module "$M/scratch" --name ExampleImported --source-url https://example.com/upstream
find "$M/scratch/skills" -name "*.yaml" -path "*.provenance*" | head
```

Expected:

- dry-run prints the planned fetch, placement, and sidecar without writing
- the real run aligns `SKILL.md`, copies every companion byte-for-byte, and writes one provenance sidecar per file
- `--kind agent|rule` imports single-file runes; `--companion` places a fetched body as a companion file

### rune adopt

```sh
rune adopt start <HTTPS-URL-or-local-path> --module "$M/scratch"   # file:// is also supported
rune adopt status                  # sessions and their progress
rune adopt next                    # the next pending blocks awaiting a verdict
rune adopt verdict SKILL.md:4 keep # verdicts: keep, adapt, cut (--note required for adapt/cut)
rune adopt finalize                # enforce verdicts, validate, seal the review record
rune adopt abandon --yes           # or: move the in-flight adoption to the tree trash
```

Expected:

- `finalize` refuses while blocks lack verdicts; the sealed review record lands beside the adopted rune
- `import` is the one-shot path; `adopt` is the reviewed path

### rune bench

Covered in depth by the dedicated Bench walkthrough (docs/walkthroughs/). Quick pass:

```sh
rune bench list                        # registry models + suites per tier (committed/user/private)
rune bench doctor                      # workspace, private tier wiring, registry, provider readiness
rune bench run --suite tier1-sample --models echo-smoke --runs 2 --version smoke
rune bench run --suite tier1-sample --models echo-smoke --runs 2 --version smoke   # full reuse, no re-execution
rune bench report --suite tier1-sample --version smoke  # rebuilds outputs purely from cache
rune bench audit                       # answers self-score, negative collisions, short tokens
rune bench run --suite tier1-sample --models qwen2.5-coder-7b --runs 1 --version smoke   # live Ollama run
rune bench dashboard                   # artifacts/dashboard.html from every suite and version
rune bench run --suite tier1-sample --models echo-smoke --runs 1 --version smoke --json
#   {"results": ..., "records": N, "reused": N, "errored": 0} — errored runs exit 1
```

`bench` in `~/.config/rune/config.yaml` is a list of workspace checkouts, each added with `rune config set bench <path>`; with no list configured, the runedeck/bench checkout is discovered automatically. The first entry is the primary (registry, dashboard); every entry contributes its suites (`suites/`, `suites/user/`, `suites/private/`), a later checkout never duplicates a stem an earlier one provides, and a suite's results and cache stay in the checkout that owns it — private-suite runs never write into the public tree. `--suite` accepts a path or a bare name with 2-char prefix matching; results, cache, and summaries are byte-compatible with the bun harness in the bench repo, and the two runners resume from each other's caches. Judged suites still run via the bun harness (`bun run bench -- run …`); `rune bench` names that clearly when pointed at one.

### rune provider

```sh
rune provider                        # name · enabled · target · plugin per provider
rune provider disable gemini         # writes providers.gemini.enabled into ./config.yaml
rune provider enable gemini
```

Expected:

- agentskills ships disabled (deploys only via `--provider agentskills`)
- assembly transforms are named rules per provider in `defaults.yaml` (`kebab-case`, `kebab-case-agents`, `remap-tools`, `strip-links`, `agents-to-toml`); a module's `config.yaml` can override the list per provider

### rune todo

```sh
T=$(mktemp -d) && cd "$T"
rune todo add "(A) verify the todo engine +rune @cli due:2026-07-25"
rune todo                              # styled list; (A) items red
rune todo ls +rune                     # filters by +project, @context, or a priority letter
rune todo obsidian                     # - [ ] … #rune @cli ⏫ 📅 2026-07-25
rune todo do 1 && rune todo            # completed item renders dim, x + date
```

Workspace aggregation: in a consumer whose `.rune` is version 2 with a `dirs:` section, `rune todo --all` lists the root plus every member (`--json` emits one workspace document). `rune todo import <file>` appends a markdown file's `- [ ]` task lines to TODO.txt.

### rune adr

```sh
mkdir -p docs/decisions
rune adr new "Try The Lifecycle" --prefix CLI
rune adr list                          # CLI-0001 · proposed
rune adr new "Replace The Lifecycle" --prefix CLI
rune adr supersede CLI-0001 CLI-0002   # status flips, cross-links both ways
rune adr index && cat docs/decisions/README.md
rune adr import ~/other-project/docs/decisions --prefix CLI --dry-run
#   plan only: each foreign ADR re-ids into THIS repo's CLI sequence
rune adr import ~/other-project/docs/decisions --prefix CLI
#   one-shot: frontmatter merges onto the ADR skeleton, body verbatim,
#   provenance sidecar records the source; foreign ids survive only there
rune adr adopt ~/other-project/docs/decisions/ARCH-0007*.md --prefix CLI
#   reviewed: stages one decision, opens a review session — continue with
rune adopt next && rune adopt verdict <block> keep && rune adopt finalize
```

Expected:

- `import` on a directory reprocesses every ADR (README.md excluded) in filename order, assigning sequential ids in the destination prefix
- `adopt` takes one file per session, exactly like AdoptArtifact; finalize validates against the decisions mdschema and seals the review record beside the ADR
- `rune validate` flags imported records whose frontmatter still misses required schema fields — import warns, validate enforces

### rune docs

```sh
printf '# Home\n\n[missing](Nope.md)\n' > docs/README.md
rune docs check                        # broken link error, exit 1
```

`rune docs dev` shells out to `mint dev` when `docs.json` exists.

### rune watch

```sh
rune watch list                        # monitored locations
rune watch add "$DECK" && rune watch remove "$DECK"
rune watch git https://github.com/runedeck/deck --ref <sha>
```

## Plumbing

### rune assemble

```sh
cd "$DECK/runes/core" && rune assemble   # transforms into build/ without deploying
```

Expected:

- assemble, deploy, and copy operate on a module root (`runes/<domain>`), not the deck root
- `build/` is rebuilt from scratch on every run
- hidden source entries (`.provenance/` review records, `.mdschema`) never assemble into it

### rune deploy

```sh
rune deploy --target "$(mktemp -d)"    # deploys what assemble produced
```

### rune copy

```sh
rune copy --source "$DECK/runes/core" --target "$(mktemp -d)"   # verbatim copy, no transforms
```

### rune find

```sh
cd "$DECK/runes/core" && rune find skill   # relevance-ranked matches from a module root
```

### rune exec

```sh
rune exec <skill> -- --help            # runs a script bundled with a skill
```

### rune launch

```sh
rune launch                            # tools with install state and profiles
rune launch llama3@ollama --dry-run    # plan shows: ollama run llama3
rune launch nonexistent@claude         # error listing known profiles
rune launch claude -- --version        # real execution: args after -- pass through
```

The invocation is `[profile@]<tool>`, user@host style: `sol@claude` is profile `sol` running at tool `claude`. Profiles live under `launch.profiles` in `~/.config/rune/config.yaml`. Environment values support `from_env` references so secrets stay out of config; an unset process variable falls back to the env file (`rune config set env <path>`, default `~/.env`).

Model routes live under `launch.models`. A profile selects one route with `model`, which keeps the provider model and context settings together. Start the local translating proxy used by the Sol and Lumo profiles, then configure the routes and profiles:

```yaml
launch:
    models:
        sol:
            id: gpt-5.6-sol
            context: 270000
        lumo:
            id: lumo-max
            context: 131072
        lumo-opencode:
            id: proton-lumo/lumo-max
            context: 131072
    profiles:
        claude:
            sol:
                model: sol
                env:
                    ANTHROPIC_BASE_URL: "http://127.0.0.1:8317"
                    ANTHROPIC_AUTH_TOKEN: { from_env: CLIPROXY_API_KEY }
                    ANTHROPIC_SMALL_FAST_MODEL: gpt-5.6-luna
            lumo:
                model: lumo
                env:
                    ANTHROPIC_BASE_URL: "http://127.0.0.1:8317"
                    ANTHROPIC_AUTH_TOKEN: { from_env: CLIPROXY_API_KEY }
                    ANTHROPIC_SMALL_FAST_MODEL: lumo-lite
        opencode:
            lumo:
                model: lumo-opencode
```

```sh
rune launch
rune launch sol@claude --dry-run
env -u CLIPROXY_API_KEY RUNE_ENV=/dev/null rune launch sol@claude --dry-run
rune launch sol@claude
```

Expected:

- dry-run reports route `sol`, model `gpt-5.6-sol`, context `270000`, and configured provenance
- generated environment includes `ANTHROPIC_MODEL=gpt-5.6-sol`, `CLAUDE_CODE_MAX_CONTEXT_TOKENS=270000`, and `CLAUDE_CODE_AUTO_COMPACT_WINDOW=270000`
- no `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` is generated because the route does not request earlier compaction
- the authentication token is redacted and a missing reference errors with the env-file path
- a newly launched Claude Code session reports the Sol model in `/status`; `! env | grep -E 'ANTHROPIC_MODEL|CLAUDE_CODE_(MAX_CONTEXT_TOKENS|AUTO_COMPACT_WINDOW)'` shows the route-derived values
- `rune launch sol@claude -- --resume` preserves the native Claude Code resume argument
- `rune launch sol@claude --tmux --dry-run` retains the interactive tmux wrapper

The `cliproxy` middleware checks a proxy-backed profile before launch. It is relevant only when the selected profile points at the local translating proxy.

```sh
rune launch sol@claude --with cliproxy --dry-run
rune launch sol@claude --with cliproxy
```

Expected: dry-run lists the proxy preflight. A responding proxy proceeds silently; an unavailable proxy prints one warning and still launches. Auto-start remains opt-in through `launch.middleware.cliproxy.command`.

### rune run

`rune run` is the supervised noninteractive counterpart to `rune launch`. It resolves the same profile, model route, environment middleware, and preflight checks, then applies a provider-specific prompt and sandbox contract. It has no timeout unless `--timeout` is present.

```sh
rune run sol@claude "Read README.md and return its first heading. Do not edit files." --dry-run
rune run sol@claude "Read README.md and return its first heading. Do not edit files."
command printf '%s\n' 'Read README.md and return its first heading. Do not edit files.' | rune run codex
rune run grok --repo . --mode read-only --prompt-file "$BRIEF"
rune --json run unsupported "Inspect only"
rune run agy "Inspect only" --timeout 4m --dry-run
```

Expected:

- dry-run reports the resolved launch plan, canonical repository, `read-only` mode, no timeout, route provenance, and redacted credentials without running preflight or the provider
- positional, file, and piped prompts produce only the final provider answer on standard output; diagnostics remain on standard error
- Claude and Grok read-only execution exposes only `Read`, `Glob`, and `Grep`; write-capable tools remain unavailable even when nested-process hardening replaces the requested permission mode
- the unsupported target returns a JSON `configuration_error`
- agy dry-run reports its native timeout and a later supervisor timeout

Exercise each configured provider in read-only mode. Do not add a timeout to Codex, Grok, Lumo, or Claude:

```sh
rune run sol@claude --repo . --mode read-only "Inspect README.md and report its first heading. Do not edit files."
rune run claude --repo . --mode read-only "Inspect README.md and report its first heading. Do not edit files."
rune run codex --repo . --mode read-only "Inspect README.md and report its first heading. Do not edit files."
rune run grok --repo . --mode read-only "Inspect README.md and report its first heading. Do not edit files."
rune run lumo@opencode --repo . --mode read-only "Inspect README.md and report its first heading. Do not edit files."
rune run agy --repo . --mode read-only --timeout 4m "Inspect README.md and report its first heading. Do not edit files."
```

Expected: each available provider returns the same heading without changing the working tree. A missing provider or unavailable subscription reports a provider or process failure rather than an empty success.

Test workspace writes only in a scratch directory:

```sh
WORK="$(mktemp -d)"
rune run codex --repo "$WORK" --mode workspace-write "Create result.txt containing only rune-run-ok."
command grep -Fx rune-run-ok "$WORK/result.txt"
```

Expected: the provider creates only the requested scratch file and grep prints `rune-run-ok`.

Confirm read-only mode refuses the same write, using a scratch directory so a regression cannot touch real work:

```sh
WORK="$(mktemp -d)"
rune run grok --repo "$WORK" --mode read-only "Create forbidden.txt containing unsafe. If you cannot write, reply exactly READ_ONLY_OK."
command ls "$WORK"
```

Expected: the directory stays empty for every provider. Claude, Grok, and OpenCode each refuse the write through their tool or permission policy rather than through the sandbox profile alone.

To verify wrapper rejection, add a temporary Claude profile whose `with` list contains `tmux`, run the command below, then remove the temporary profile:

```sh
rune run wrapped@claude "Inspect only" --dry-run
```

Expected: automated execution rejects the tmux wrapper before preflight or process creation and directs the user to an unwrapped profile or `rune launch`. Repeat with a temporary Docker wrapper when Docker middleware is configured.

## Reference

Backups: `~/Data/Claude/backups/runedeck-*.tgz`. ADRs: `docs/decisions/` in both repos. Cleanup: `rune target -` restores your previous target binding after the temp-dir steps.
