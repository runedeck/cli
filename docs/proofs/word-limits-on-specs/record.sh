#!/usr/bin/env bash
# Driver for a recorded acceptance proof. asciinema records this script.
# The grammar lives in ../driver.sh; edit only the scenes at the end.
# shellcheck source=../driver.sh
. "$(dirname "$0")/../driver.sh"

printf '\033[2J\033[H'

# Off camera: a scratch repository with one change and one canonical
# specification, and a clean CHANGELOG.md. RUNE names the binary under
# test. Each scene edits one file and runs the checker on it.
RUNE=${RUNE:-rune}
PROOF_HOME=$(mktemp -d)
REPO="$PROOF_HOME/repo"
mkdir -p "$REPO/docs/specs/rune-name-search" "$REPO/docs/changes/tighten-search-ranking/specs/rune-name-search"
cd "$REPO" || exit 1
git init --quiet
cat > docs/specs/rune-name-search/spec.md <<'SPEC'
# Search Specification

## Purpose

Find runes by name.

## Requirements

### Requirement: Search ranks exact names first

The system MUST list an exact name match before every partial match.

#### Scenario: Exact and partial matches

- **WHEN** a query equals one rune name and starts another
- **THEN** the exact match is listed first
SPEC
cat > docs/changes/tighten-search-ranking/proposal.md <<'PROPOSAL'
---
adr: "docs/decisions/ADR-XXXX.md"
status: proposed
---
# Tighten search ranking

## Why

Ranking ties.

## What Changes

- Tie-break by domain.

## Capabilities

- rune-name-search (new or modified)

## Impact

- The search command.
PROPOSAL
printf '# Tasks\n\n- [ ] 1.1 Rank by domain\n' > docs/changes/tighten-search-ranking/tasks.md
cat > docs/changes/tighten-search-ranking/specs/rune-name-search/spec.md <<'DELTA'
## ADDED Requirements

### Requirement: Ties break by domain

The system MUST list a core rune before a rune of another domain when their names match equally.

#### Scenario: Two runes match equally

- **WHEN** two runes match a query equally
- **THEN** the core rune is listed first
DELTA
cat > CHANGELOG.md <<'LOG'
# Changelog

## [Unreleased]

### Added

- Add domain tie-breaking to search ranking
LOG

# wall WORDS: a requirement statement of WORDS words, one MUST per sentence.
wall() {
    local n=$1 text=""
    while [ "$n" -gt 0 ]; do
        text="${text}The system MUST keep this rule. "
        n=$((n - 6))
    done
    printf '%s' "$text"
}

scenario "Requirement runs to a wall of text"
comment "A 191-word requirement statement in a delta is an error at its heading line."
python3 - "$(wall 191)" <<'PY'
import pathlib, sys
path = pathlib.Path("docs/changes/tighten-search-ranking/specs/rune-name-search/spec.md")
path.write_text(path.read_text().replace("The system MUST list a core rune before a rune of another domain when their names match equally.", sys.argv[1].strip()))
PY
run "$RUNE spec validate --source . || true"
expect "delta-requirement-too-long"
expect "spec.md:3: requirement statement has 19[0-9] words; the limit is 100"

scenario "Step lists a command"
comment "Inline code does not count: a step with a long command and ten words of prose passes."
python3 - "$(wall 191)" <<'PY'
import pathlib, sys
path = pathlib.Path("docs/changes/tighten-search-ranking/specs/rune-name-search/spec.md")
text = path.read_text().replace(sys.argv[1].strip(), "The system MUST list a core rune before a rune of another domain when their names match equally.")
text = text.replace("- **THEN** the core rune is listed first", "- **THEN** `rune search --domain core --domain meta --sort name --limit 20 --format table --no-color --verbose --json --quiet --source . --cast all` lists the core rune first with ten words of prose")
path.write_text(text)
PY
run "$RUNE spec validate --source ."
expect "^specification validation passed$"

scenario "Entry is a paragraph"
comment "A changelog line of 1,400 characters is a paragraph. One line per change, 200 characters."
python3 - <<'PY'
import pathlib
path = pathlib.Path("CHANGELOG.md")
path.write_text(path.read_text() + "- Change the ranking so that " + "the core domain wins every tie and the reader learns why, " * 24 + "\n")
PY
run "$RUNE docs check || true"
expect "CHANGELOG.md:8: entry-length: 14[0-9][0-9] characters; the limit is 200"

scenario "Entry starts with the command name"
comment "A change line starts with a verb, never with a code span."
python3 - <<'PY'
import pathlib
path = pathlib.Path("CHANGELOG.md")
lines = path.read_text().splitlines()[:7]
lines.append("- `rune search` ranks core first")
path.write_text("\n".join(lines) + "\n")
PY
run "$RUNE docs check || true"
expect "CHANGELOG.md:8: entry-verb"

scenario "Repository keeps no changelog"
comment "No CHANGELOG.md is no error: the link check alone decides."
run "rm CHANGELOG.md; $RUNE docs check"
expect "links resolve"
expect_not "changelog"

scenario "Two-word capability"
comment "A change id and a capability need three hyphen-separated words by default."
mkdir -p docs/changes/add-player/specs/cast-player
printf -- '---\nadr: "docs/decisions/ADR-XXXX.md"\nstatus: proposed\n---\n# Add player\n\n## Why\n\nPlay.\n\n## What Changes\n\n- Play.\n\n## Capabilities\n\n- cast-player (new or modified)\n\n## Impact\n\n- The page.\n' > docs/changes/add-player/proposal.md
printf '# Tasks\n\n- [ ] 1.1 Play\n' > docs/changes/add-player/tasks.md
printf '## ADDED Requirements\n\n### Requirement: Player pauses\n\nThe player MUST pause on a click.\n\n#### Scenario: Reader clicks\n\n- **WHEN** the reader clicks the screen\n- **THEN** playback pauses\n' > docs/changes/add-player/specs/cast-player/spec.md
run "$RUNE spec validate --source . || true"
expect "delta-capability-name-short.*capability 'cast-player' has 2 words"
expect "change-name-short.*change 'add-player' has 2 words"

scenario "Repository turns the rule off"
comment "spec.min_name_words: 0 in the repository config switches the floor off."
printf 'spec:\n    min_name_words: 0\n' > config.yaml
run "$RUNE spec validate --source ."
expect "^specification validation passed$"
