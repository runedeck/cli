#!/usr/bin/env bash
# Driver for a recorded acceptance proof. asciinema records this script.
# The grammar lives in ../driver.sh; edit only the scenes at the end.
# shellcheck source=../driver.sh
. "$(dirname "$0")/../driver.sh"

printf '\033[2J\033[H'

# Off camera: a scratch deck with one domain and one skill, and a consumer
# whose .rune names it. rune install deploys the deck, so every provider
# directory carries a real .manifest before the first scene. RUNE names
# the binary under test. DECK_SOURCE names the deck the fixture copies
# from, default the deck two levels above this repository.
RUNE=${RUNE:-rune}
DECK_SOURCE=${DECK_SOURCE:-$(cd "$(dirname "$0")/../../.." && pwd)/../deck}
PROOF_HOME=$(mktemp -d)
DECK="$PROOF_HOME/deck"
CONSUMER="$PROOF_HOME/consumer"
mkdir -p "$DECK/runes/core/skills" "$DECK/docs/changes" "$CONSUMER"
cp -f "$DECK_SOURCE/deck.yaml" "$DECK/deck.yaml"
cp -f "$DECK_SOURCE/runes/core/module.yaml" "$DECK/runes/core/module.yaml"
cp -R -f "$DECK_SOURCE/runes/core/skills/RTK" "$DECK/runes/core/skills/RTK"
printf 'version: 1\nsources:\n  deck:\n    local: %s\nrunes:\n  deck:\n    include:\n    - core/skills/RTK\n' "$DECK" > "$CONSUMER/.rune"
cd "$CONSUMER" || exit 1
preflight "$RUNE" install

scenario "Draft skill in a consumer with deployed providers"
comment "A draft goes into every provider directory that carries a .manifest, and never into the .manifest itself."
run "$RUNE draft skill ReviewSpec"
expect "draft \.claude/skills/ReviewSpec/SKILL\.md"
expect "draft \.codex/skills/ReviewSpec/SKILL\.md"
expect "registered in \.drafts"
run "cat .drafts"
expect "path: \.claude/skills/ReviewSpec/SKILL\.md"
expect "kind: skill"
run "grep -c ReviewSpec .claude/.manifest || true"
expect "^0$"

scenario "Registered draft under a managed directory"
comment "Doctor lists the draft with its age and reports no orphan for it."
run "$RUNE doctor --target ."
expect "draft +ReviewSpec skill [0-9]+ days"
expect_not "orphan +\."

scenario "Draft name already registered with another kind"
comment "One name means one kind: --drop and promote act by name."
run "$RUNE draft rule ReviewSpec || true"
expect "one name means one kind"

scenario "Promote refuses before touching anything"
comment "A change id needs three words, and a domain is one identifier."
run "$RUNE promote ReviewSpec --domain core --change review-spec || true"
expect "at least three hyphen-separated lowercase words"
run "$RUNE promote ReviewSpec --domain ../escape --change review-spec-interaction || true"
expect "domain '\.\./escape' must be"
run "$RUNE draft --list"
expect "ReviewSpec +skill"

scenario "Promote a draft skill"
comment "The rune moves into the deck, spec propose opens the change, and the provider copies go."
run "$RUNE promote ReviewSpec --domain core --change review-spec-interaction"
expect "promoted ReviewSpec to .*/deck/runes/core/skills/ReviewSpec/SKILL\.md"
expect "removed \.claude/skills/ReviewSpec/SKILL\.md"
expect "change stub at .*/docs/changes/review-spec-interaction"
run "cat $DECK/docs/changes/review-spec-interaction/proposal.md" "cat \"$DECK/docs/changes/review-spec-interaction/proposal.md\""
expect "runes/core/skills/ReviewSpec/SKILL\.md"
run "$RUNE draft --list"
expect "^no drafts$"

scenario "Draft file removed by hand"
comment "A register entry whose file is gone is a stale draft, and --verify exits nonzero."
run "$RUNE draft rule Terse"
expect "draft \.claude/rules/Terse\.md"
run "rm .claude/rules/Terse.md"
run "$RUNE doctor --target . --verify || echo \"exit \$?\""
expect "stale +Terse rule"
expect "^exit 1$"
run "$RUNE draft --drop Terse"
expect "removed \.claude/rules/Terse\.md"
run "$RUNE doctor --target . --verify && echo clean"
expect "^clean$"
