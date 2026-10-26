#!/bin/sh
# Build a reviewed module at $1: a git repository whose Greeting skill
# passed an adoption review that kept every block, committed once.
set -eu

fixture=$(cd "$(dirname "$0")" && pwd)
repo=$1

sh "$fixture/open-session.sh" "$repo" >/dev/null
rune adopt verdict SKILL.md:5 keep --root "$repo" >/dev/null
rune adopt finalize --root "$repo" --reviewer 'Proof Reviewer' >/dev/null
git -C "$repo" add -A
git -C "$repo" -c user.name='Proof Reviewer' -c user.email=proof@example.test \
    -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -qm 'adopt Greeting'
echo "reviewed skills/Greeting in $repo"
