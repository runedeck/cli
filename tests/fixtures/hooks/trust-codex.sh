#!/bin/sh
# Trust one hook of one Codex event the way `/hooks` records it: the
# sha256 of the hook's identity, keyed by table, event label, group, and
# handler, appended to the Codex config. Not for SessionEnd, whose
# timeout Codex clamps. WHICH picks the event's first rune entry, the
# default, `last`, the one an install added after an earlier one, or the
# hook whose command is WHICH when it starts with a slash.
#
# usage: trust-codex.sh HOME EVENT LABEL [WHICH], such as PreToolUse pre_tool_use
home=$1
event=$2
label=$3
command=
case ${4:-first} in
last) index=-1 ;;
/*) index=0 command=$4 ;;
*) index=0 ;;
esac
table="$home/.codex/hooks.json"
found=$(jq -c --arg e "$event" --arg l "$label" --argjson i "$index" --arg c "$command" '
  [.hooks[$e] | to_entries[] | .key as $g | .value.matcher as $m
   | .value.hooks | to_entries[]
   | select(if $c == "" then (.value.command | contains("rune hook run")) else .value.command == $c end)
   | {g: $g, h: .key,
      id: ({event_name: $l,
            hooks: [{async: (.value.async // false), command: .value.command,
                     timeout: (.value.timeout // 600), type: "command"}]}
           + (if $m then {matcher: $m} else {} end))}][$i]' "$table")
group=$(printf '%s' "$found" | jq -r .g)
handler=$(printf '%s' "$found" | jq -r .h)
hash=$(printf '%s' "$found" | jq -cS .id | tr -d '\n' | shasum -a 256 | cut -d' ' -f1)
printf '[hooks.state."%s:%s:%s:%s"]\ntrusted_hash = "sha256:%s"\n' \
  "$table" "$label" "$group" "$handler" "$hash" >> "$home/.codex/config.toml"
