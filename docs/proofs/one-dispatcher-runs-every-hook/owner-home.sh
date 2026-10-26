#!/bin/sh
# Write a synthetic home in the shape of the owner's live legacy
# dispatcher: the dispatcher script at ~/.config/just/hook, its Stop.d
# parts (tmux attention, turn wrap, turn snapshot), the programs the
# parts and the handlers run, a rune config with the tmux, turn, and
# capture handlers on turn.finish, and a Claude Stop entry that runs the
# dispatcher. The tmux and turn programs write one line each to
# ran.log in the home, so a scene sees what ran.
#
# Usage: sh owner-home.sh <home>
set -eu

home=$1
just="$home/.config/just"
stop="$just/hooks/Stop.d"
mkdir -p "$stop" "$just/claude/turn" "$home/.local/bin" "$home/.config/rune" "$home/.claude"

cat >"$just/hook" <<'EOF'
#!/bin/sh
# Claude Code hook dispatcher.
set -u

event="${1:?usage: hook <Event>}"
dir="$HOME/.config/just/hooks/$event.d"
[ -d "$dir" ] || exit 0

payload=$(cat 2>/dev/null || true)

for part in "$dir"/*.sh; do
    [ -e "$part" ] || break
    printf '%s' "$payload" | sh "$part"
    rc=$?
    [ "$rc" -eq 0 ] || exit "$rc"
done
exit 0
EOF
chmod 755 "$just/hook"

cat >"$stop/10-tmux-attention.sh" <<'EOF'
#!/bin/sh
exec "$HOME/.local/bin/claude-tmux-status" attention
EOF
cat >"$stop/20-turn-wrap.sh" <<'EOF'
#!/bin/sh
exec sh "$HOME/.config/just/claude/turn/wrap"
EOF
cat >"$stop/30-turn-snapshot.sh" <<'EOF'
#!/bin/sh
exec sh "$HOME/.config/just/claude/turn/checkpoint" snapshot
EOF

cat >"$home/.local/bin/claude-tmux-status" <<'EOF'
#!/bin/sh
cat >/dev/null
echo "tmux $*" >>"$HOME/ran.log"
EOF
cat >"$home/.local/bin/session-sync" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod 755 "$home/.local/bin/claude-tmux-status" "$home/.local/bin/session-sync"

cat >"$just/claude/turn/checkpoint" <<'EOF'
#!/bin/sh
cat >/dev/null
echo "turn $*" >>"$HOME/ran.log"
EOF
cat >"$just/claude/turn/wrap" <<'EOF'
#!/bin/sh
cat >/dev/null
EOF

cat >"$home/.config/rune/config.yaml" <<'EOF'
hooks:
  handlers:
    - id: tmux
      exec: [rune-hook-tmux-status]
      events: [turn.finish]
    - id: turn
      exec: [rune-hook-turn-checkpoint]
      events: [turn.finish]
    - id: capture
      exec: [rune-hook-session-capture]
      events: [turn.finish]
EOF

cat >"$home/.claude/settings.json" <<'EOF'
{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"sh $HOME/.config/just/hook Stop"}]}]}}
EOF
