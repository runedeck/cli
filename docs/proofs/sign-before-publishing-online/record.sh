#!/usr/bin/env bash
# Driver for a recorded acceptance proof. asciinema records this script.
# The grammar lives in ../driver.sh; edit only the scenes at the end.
# shellcheck source=../driver.sh
. "$(dirname "$0")/../driver.sh"

printf '\033[2J\033[H'

# shellcheck source=../signing-fixture.sh
. "$(dirname "$0")/../signing-fixture.sh"

# publish NAME COMMIT: point origin/change/NAME at COMMIT, as a fetch would
# after a push. The proof has no network; the remote-tracking ref is the
# only thing the guard reads.
publish() {
    git update-ref "refs/remotes/origin/change/$1" "$2"
}

TOP=$(jj log --no-graph -r 'bookmarks(exact:"change/top")' -T commit_id)
BASE=$(jj log --no-graph -r 'bookmarks(exact:"change/base")' -T commit_id)
printf 'bookmark: change/top [move sideways from 0 to %s]\nexit=0\n' "$TOP" > "$PROOF_HOME/top.log"

scenario "Head already pushed"
comment "origin/change/top already holds the head: signing it in place would rewrite a pushed tip."
publish top "$TOP"
run "$RUNE sign queue change/top --receipt $PROOF_HOME/top.log || true"
expect "already published on origin"
expect "Sign before you push"
run "$RUNE sign queue"
expect "is empty"

scenario "Remote moved past the head"
comment "The remote is a descendant of the head: still published, still refused."
run "jj new change/top -m 'feat: later' && jj bookmark set change/later -r @ && jj new" "jj new change/top -m 'feat: later' >/dev/null 2>&1 && jj bookmark set change/later -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; echo moved"
LATER=$(jj log --no-graph -r 'bookmarks(exact:"change/later")' -T commit_id)
publish top "$LATER"
run "$RUNE sign queue change/top --receipt $PROOF_HOME/top.log || true"
expect "already published on origin"

scenario "Remote holds another commit"
comment "origin/change/top points elsewhere: the head is unpublished and the request is recorded."
publish top "$BASE"
run "$RUNE sign queue change/top --receipt $PROOF_HOME/top.log"
expect "^queued .* change/top"
run "$RUNE sign drop change/top"
expect "^dropped "

scenario "jj refuses an immutable commit"
comment "A head inside immutable_heads() is jj's own word for published. The queue reports it and never adds --ignore-immutable."
fresh pinned
printf '[revset-aliases]\n"immutable_heads()" = "bookmarks(exact:\\"change/pinned\\")"\n' >> "$JJ_CONFIG"
run "$RUNE sign queue change/pinned --receipt pinned.log"
expect "^queued "
run "$RUNE sign next || true"
expect "refused: change/pinned is immutable"
expect "Sign before you push"
python3 - "$JJ_CONFIG" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
path.write_text(path.read_text().split("[revset-aliases]")[0])
PY
run "$RUNE sign drop change/pinned"
expect "^dropped "

scenario "Pinentry timed out once"
comment "The first attempt times out. The retry carries the focus hint."
fresh retried
printf Timeout > "$FAKE_GPG_FAIL_ONCE"
run "$RUNE sign queue change/retried --receipt retried.log"
expect "^queued "
run "$RUNE sign next"
expect "Keep the pinentry focused"
expect "^signed change/retried"

scenario "Head signed"
comment "A signed head prints the publication command and runs nothing."
fresh guarded
run "$RUNE sign queue change/guarded --receipt guarded.log"
expect "^queued "
run "$RUNE sign next"
expect "^signed change/guarded [0-9a-f]+"
expect "publish with: jj git push --remote origin --bookmark change/guarded"
