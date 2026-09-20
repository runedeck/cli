#!/usr/bin/env bash
# The signing proof fixture, sourced by every rune sign proof after
# ../driver.sh: a colocated jj repository with change/base and change/top,
# the repository's KEYS file, and a gpg stand-in that honors jj's signing
# contract without an agent, so the proof runs on a machine without the
# owner's card. RUNE names the binary under test, KEYS_FILE the KEYS file.
# The shell ends inside the repository. `fresh NAME` adds a bookmark on
# the base with one file and a passing receipt.

RUNE=${RUNE:-rune}
KEYS_FILE=${KEYS_FILE:-$(cd "$(dirname "$0")/../../.." && pwd)/KEYS}
PROOF_HOME=$(mktemp -d)
export RUNE_STATE_DIR="$PROOF_HOME/state" RUNE_NO_NOTIFY=1 FAKE_GPG_FAIL_ONCE="$PROOF_HOME/fail-once"
export FAKE_GPG_SLEEP="$PROOF_HOME/sleep"
export GNUPGHOME="$PROOF_HOME/gnupg"
mkdir -p "$GNUPGHOME" && chmod 700 "$GNUPGHOME"
: > "$FAKE_GPG_FAIL_ONCE" "$FAKE_GPG_SLEEP"
cat > "$PROOF_HOME/fake-gpg" <<'GPG'
#!/bin/sh
case " $* " in
  *" -abu "*)
    if [ -s "$FAKE_GPG_SLEEP" ]; then sleep "$(cat "$FAKE_GPG_SLEEP")"; : > "$FAKE_GPG_SLEEP"; fi
    if [ -s "$FAKE_GPG_FAIL_ONCE" ]; then
      message=$(cat "$FAKE_GPG_FAIL_ONCE"); : > "$FAKE_GPG_FAIL_ONCE"
      echo "gpg: signing failed: $message" >&2; exit 2
    fi
    cat >/dev/null
    printf -- '-----BEGIN PGP SIGNATURE-----\nfake %s\n-----END PGP SIGNATURE-----\n' "${FAKE_GPG_KEY:-29DD2145CE7A818929459B2649F08103D3DA399E}"
    ;;
  *" --verify "*)
    cat >/dev/null
    key="${FAKE_GPG_KEY:-29DD2145CE7A818929459B2649F08103D3DA399E}"
    printf '[GNUPG:] NEWSIG\n[GNUPG:] GOODSIG %s Owner Test <owner@example.com>\n[GNUPG:] VALIDSIG %s 2026-09-15 1789482678 0 4 0 1 10 00 %s\n' "$key" "$key" "$key"
    ;;
  *) exit 3 ;;
esac
GPG
chmod 755 "$PROOF_HOME/fake-gpg"
REPO="$PROOF_HOME/repo"
jj git init --colocate "$REPO" >/dev/null 2>&1
python3 -c 'import shutil,sys; shutil.copy(sys.argv[1], sys.argv[2])' "$KEYS_FILE" "$REPO/KEYS"
printf '[signing]\nbackend = "gpg"\nkey = "49F08103D3DA399E"\n[signing.backends.gpg]\nprogram = "%s"\n' "$PROOF_HOME/fake-gpg" > "$REPO/.jjconfig.toml"
export JJ_CONFIG="$REPO/.jjconfig.toml" JJ_USER="Model One" JJ_EMAIL="model-one@example.com"
export XDG_CONFIG_HOME="$PROOF_HOME/xdg"
cd "$REPO" || exit 1
jj describe -m 'feat: base' >/dev/null 2>&1
jj bookmark set change/base -r @ >/dev/null 2>&1
jj new -m 'feat: top' >/dev/null 2>&1 && printf 'top\n' > top.txt
jj bookmark set change/top -r @ >/dev/null 2>&1
jj new >/dev/null 2>&1
# The ceremony reads KEYS from the protected branch on the remote. The
# proof has no network, so origin is a bare repository beside the clone,
# with main at the base.
git init --quiet --bare "$PROOF_HOME/origin.git"
git remote add origin "$PROOF_HOME/origin.git"
git push --quiet origin "$(jj log --no-graph -r 'bookmarks(exact:"change/base")' -T commit_id):refs/heads/main" >/dev/null 2>&1
git fetch --quiet origin >/dev/null 2>&1
# fresh NAME: a new bookmark on the base with one file and a passing receipt.
fresh() {
    jj new change/base -m "feat: $1" >/dev/null 2>&1 && printf '%s\n' "$1" > "$1.txt"
    jj bookmark set "change/$1" -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1
    local head
    head=$(jj log --no-graph -r "bookmarks(exact:\"change/$1\")" -T commit_id)
    printf 'bookmark: change/%s [to %s]\nexit=0\n' "$1" "$head" > "$1.log"
}
