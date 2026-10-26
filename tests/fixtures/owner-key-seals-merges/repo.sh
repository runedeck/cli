# shellcheck shell=sh
# Sourced by the owner-key-seals-merges proof scenes from the scene
# directory. It builds a bare `origin.git` and a clone `repo` whose branch
# `change/demo` holds one pushed commit above `main`, commits the fixture
# KEYS, and signs through fake-gpg as the owner's key. The scene gets its
# own copy of fake-gpg with the executable bit set, because a checkout may
# drop that bit. Global and system git config stay out, and GNUPGHOME is an
# empty keyring. The shell ends inside `repo`.
fixture=$RUNE_PROOF_ROOT/tests/fixtures/owner-key-seals-merges
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GNUPGHOME="$PWD/gnupg"
mkdir -m 700 gnupg
cp -f "$fixture/fake-gpg" fake-gpg
chmod +x fake-gpg
gpg_program=$PWD/fake-gpg
git init -q --bare origin.git
git init -q -b main repo
cd repo || exit 1
git config user.name 'Owner Test'
git config user.email owner@example.com
git config gpg.program "$gpg_program"
git config user.signingkey AAAA0000AAAA0000AAAA0000AAAA0000AAAA0000
cp -f "$fixture/KEYS" KEYS
git add KEYS
git commit -q -m 'chore: add KEYS'
git remote add origin ../origin.git
git push -q origin main
git switch -q -c change/demo
printf 'demo\n' > demo.txt
git add demo.txt
git commit -q -m 'feat: demo'
git push -q -u origin change/demo
