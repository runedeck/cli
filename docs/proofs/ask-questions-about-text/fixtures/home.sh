#!/bin/sh
# Make a scene-local home for `HOME=home rune ask ...`: the question
# folder `questions` with the session-shape set, and a config whose
# `ask:` section names that folder, then holds every argument as one
# more line, indented under `ask:`. An argument that starts with
# `questions:` replaces the default folder list. The word STUB in an
# argument becomes the stub's base URL, read from stub.port. The config
# is printed.
#
#   sh fixtures/home.sh "routes:" "  jev:" "    base_url: STUB"
set -eu

here=$(cd "$(dirname "$0")" && pwd)
mkdir -p home/.config/rune questions
cp "$here/session-shape.json" questions/session-shape.json
if [ -s stub.port ]; then
    stub="http://127.0.0.1:$(cat stub.port)"
else
    stub=STUB
fi
folders='questions: [questions]'
for line in "$@"; do
    case $line in
    questions:*) folders=$line ;;
    esac
done
{
    printf 'ask:\n  %s\n' "$folders"
    for line in "$@"; do
        case $line in
        questions:*) ;;
        *) printf '  %s\n' "$line" | sed "s|STUB|$stub|" ;;
        esac
    done
} >home/.config/rune/config.yaml
cat home/.config/rune/config.yaml
