#!/bin/sh
# Fire the rune entry that install wrote into a home's table for one
# harness and event, with that harness's payload on stdin. Each extra
# argument is a NAME=VALUE that the firing's environment gets on top of
# HOME. Then print what the dispatcher log says of each handler at that
# firing, and the lines the programs wrote to ran.log.
#
# Usage: RUNE_STATE_DIR=<state> sh fire.sh <home> <harness> <event> [NAME=VALUE...]
set -eu

home=$(cd "$1" && pwd)
harness=$2
event=$3
shift 3
case $harness in
claude) table="$home/.claude/settings.json" ;;
*) table="$home/.$harness/hooks.json" ;;
esac
case $event in
PreToolUse | PostToolUse) tool=',"tool_name":"Bash","tool_input":{"command":"ls"}' ;;
*) tool= ;;
esac
command=$(jq -r --arg event "$event" \
    '.hooks[$event][].hooks[].command | select(contains(" hook run "))' "$table")
rm -f "$RUNE_STATE_DIR/hooks/run.log" "$home/ran.log"
set -f
# shellcheck disable=SC2086 # the command is split into its words on purpose
printf '{"hook_event_name":"%s","session_id":"s1","cwd":"/"%s}' "$event" "$tool" |
    env "$@" HOME="$home" $command >/dev/null
sed 1d "$RUNE_STATE_DIR/hooks/run.log"
if [ -e "$home/ran.log" ]; then
    cat "$home/ran.log"
fi
