#!/bin/sh
# Open an adoption review of the Greeting skill in a new git repository at
# $1 and keep every block but the last, so SKILL.md:5 still waits for its
# verdict.
set -eu

fixture=$(cd "$(dirname "$0")" && pwd)
repo=$1

mkdir -p "$repo"
cp -f "$fixture/module/module.yaml" "$repo/module.yaml"
git -C "$repo" init -q
rune adopt start "$fixture/upstream/greeting" --module "$repo" --name Greeting >/dev/null
for block in Notes.md:1 Notes.md:2 SKILL.md:1 SKILL.md:2 SKILL.md:3 SKILL.md:4; do
    rune adopt verdict "$block" keep --root "$repo" >/dev/null
done
echo "SKILL.md:5 pending in $repo"
