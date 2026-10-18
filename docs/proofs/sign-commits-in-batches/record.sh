#!/usr/bin/env bash
# Driver for a recorded acceptance proof. asciinema records this script.
# The grammar lives in ../driver.sh; edit only the scenes at the end.
# shellcheck source=../driver.sh
. "$(dirname "$0")/../driver.sh"

printf '\033[2J\033[H'

# shellcheck source=../signing-fixture.sh
. "$(dirname "$0")/../signing-fixture.sh"

# two_owners: a signer whose card is slow holds the lock while a second
# signer arrives.
two_owners() {
    printf 3 > "$FAKE_GPG_SLEEP"
    "$RUNE" sign next > first.out 2>&1 &
    local first=$!
    sleep 1
    "$RUNE" sign next
    echo "exit=$?"
    wait "$first"
    cat first.out
}

# crash_claim NAME: the request for change/NAME looks claimed by a signer
# that died after jj signed the head but before the queue recorded it.
crash_claim() {
    jj sign -r "change/$1" >/dev/null 2>&1
    python3 - "$RUNE_STATE_DIR/sign-queue/requests" "change/$1" <<'PY'
import json, pathlib, sys
for path in pathlib.Path(sys.argv[1]).glob("*.json"):
    record = json.loads(path.read_text())
    if record["bookmark"] == sys.argv[2]:
        record["claim"] = {"pid": 999999, "started_at": "2026-09-15T00:00:00Z"}
        path.write_text(json.dumps(record))
PY
}

# plant_hostile: a repository-scope jj config that names a signing program
# under the session's control.
plant_hostile() {
    printf '#!/bin/sh\ntouch "%s/pwned"\nexit 1\n' "$PROOF_HOME" > "$PROOF_HOME/hostile"
    chmod 755 "$PROOF_HOME/hostile"
    # jj creates the repository's config id on first use of its config path.
    jj config path --repo >/dev/null 2>&1 || true
    local repo_config
    repo_config="$XDG_CONFIG_HOME/jj/repos/$(cat .jj/repo/config-id)"
    mkdir -p "$repo_config"
    printf '[signing.backends.gpg]\nprogram = "%s"\n' "$PROOF_HOME/hostile" > "$repo_config/config.toml"
    jj config list --repo signing
}

BASE=$(jj log --no-graph -r 'bookmarks(exact:"change/base")' -T commit_id)
TOP=$(jj log --no-graph -r 'bookmarks(exact:"change/top")' -T commit_id)
printf 'bookmark: change/base [move sideways from 0 to %s]\ndryrun-exit=0\n' "$BASE" > base.log
printf 'bookmark: change/top [move sideways from 0 to %s]\nexit=0\n' "$TOP" > top.log
printf 'bookmark: change/base [move sideways from 0 to %s]\ndryrun-exit=1\n' "$BASE" > failing.log

scenario "Queue is empty"
run "$RUNE sign queue; $RUNE sign next"
expect "^the signing queue at .*/sign-queue is empty$"
expect "^nothing to sign in .*/sign-queue$"

scenario "Head queued after a passing dry run"
run "$RUNE sign queue change/top --receipt top.log"
expect "run \`rune sign next\` at the key"
run "$RUNE sign submit change/base --receipt base.log"
expect "^queued "

scenario "Receipt reports a failure"
run "$RUNE sign queue change/base --receipt failing.log; echo \"exit=\$?\""
expect "not clean"
expect "^exit=2$"

scenario "Stack of two"
run "$RUNE sign queue"
expect "^current .* change/base"
expect "^blocked .* change/top"

scenario "Pinentry timed out once"
run "printf Timeout > \"\$FAKE_GPG_FAIL_ONCE\"; $RUNE sign next" "printf Timeout > \"$FAKE_GPG_FAIL_ONCE\"; $RUNE sign next 2>&1"
expect "timed out \(attempt 1 of 3\)"
expect "^signed change/base"
run "jj log --no-graph -r 'bookmarks(exact:\"change/base\")' -T 'if(signature, signature.status(), \"unsigned\")'"
expect "^good$"

scenario "Head rewritten by signing an ancestor"
run "$RUNE sign queue"
expect "^signed .* change/base"
expect "^current .* change/top"

scenario "Every current request signs in order"
run "$RUNE sign all"
expect "^signed change/top"

scenario "Show and JSON"
run "$RUNE sign show change/top"
expect "^state +signed"
run "$RUNE sign queue --json | head -c 160"
expect '"state": "signed"'

scenario "Head amended after the request"
run "fresh side; $RUNE sign queue change/side --receipt side.log"
expect "^queued "
run "jj describe -r change/side -m 'feat: side, amended' >/dev/null; $RUNE sign show change/side"
expect "^state +stale"

scenario "Stale request in the queue"
run "$RUNE sign next; echo exit=\$?"
expect "skipped: stale"
expect "^exit=1$"

scenario "Request withdrawn"
run "$RUNE sign drop change/side"
expect "^dropped "

scenario "Pinentry cancelled"
run "fresh cancel; $RUNE sign queue change/cancel --receipt cancel.log >/dev/null; printf 'Operation cancelled' > \"\$FAKE_GPG_FAIL_ONCE\"; $RUNE sign next; echo exit=\$?"
expect "^cancelled"
expect "^exit=1$"
run "$RUNE sign show change/cancel"
expect "^state +current"

scenario "Signature from another key"
run "FAKE_GPG_KEY=0000000000000000 $RUNE sign next; echo exit=\$?"
expect "not in KEYS"
expect "^exit=1$"
run "$RUNE sign show change/cancel"
expect "^state +failed"

scenario "Two owners at once"
run "$RUNE sign queue --prune >/dev/null; fresh two; $RUNE sign queue change/two --receipt two.log >/dev/null; two_owners"
expect "another signer holds"
expect "^exit=2$"
expect "^signed change/two"

scenario "Signer crashed after signing"
run "fresh late; $RUNE sign queue change/late --receipt late.log >/dev/null; crash_claim late; $RUNE sign next"
expect "^signed change/late"

scenario "Repository names a hostile signing program"
run "plant_hostile"
expect "hostile"
run "fresh safe; $RUNE sign queue change/safe --receipt safe.log >/dev/null; $RUNE sign next; test -e \"\$PROOF_HOME/pwned\" && echo 'hostile program ran' || echo 'hostile program never ran'"
expect "^signed change/safe"
expect "hostile program never ran"

scenario "Repository aliases a template keyword"
run "printf '[template-aliases]\nvalue = %s\n' \"'\\\"forged\\\"'\" >> \"\$XDG_CONFIG_HOME/jj/repos/\$(cat .jj/repo/config-id)/config.toml\"; fresh alias; $RUNE sign queue change/alias --receipt alias.log >/dev/null; $RUNE sign next; echo exit=\$?"
expect "template-aliases.value"
expect "^exit=2$"

scenario "Subcommand followed by a ceremony flag"
run "$RUNE sign next --tag v1; echo exit=\$?" "$RUNE sign next --tag v1 2>&1; echo exit=\$?"
expect -- "--tag"
expect "^exit=2$"

comment "Done"
sleep 1
