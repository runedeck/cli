#!/usr/bin/env bash
# Driver for a recorded acceptance proof. asciinema records this script.
# The grammar lives in ../driver.sh; edit only the scenes at the end.
# shellcheck source=../driver.sh
. "$(dirname "$0")/../driver.sh"

printf '\033[2J\033[H'

# Off camera: a scratch HOME whose rune config carries three claude
# profiles, and a fake `claude` on PATH that prints the arguments it
# received, one per line. No model is called. RUNE names the binary
# under test.
RUNE=${RUNE:-rune}
PROOF_HOME=$(mktemp -d)
export HOME="$PROOF_HOME"
mkdir -p "$PROOF_HOME/.config/rune" "$PROOF_HOME/bin" "$PROOF_HOME/repo"
cat > "$PROOF_HOME/bin/claude" <<'SH'
#!/usr/bin/env bash
# Fake claude: echo the argv so the proof shows what reached the tool.
cat >/dev/null
printf 'argv:'
for arg in "$@"; do printf ' %s' "$arg"; done
printf '\n'
SH
chmod +x "$PROOF_HOME/bin/claude"
export PATH="$PROOF_HOME/bin:$PATH"
cat > "$PROOF_HOME/.config/rune/config.yaml" <<'YAML'
launch:
  profiles:
    claude:
      teams-off:
        model: sol
        args:
          - "--settings"
          - '{"env":{"CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS":"0"}}'
      widening:
        model: sol
        args:
          - "--permission-mode"
          - "acceptEdits"
          - "--settings"
          - '{"env":{"X":"1"},"permissions":{"allow":["Bash"]}}'
      plain:
        model: sol
        args:
          - "--verbose"
YAML
cd "$PROOF_HOME/repo" || exit 1
preflight "$RUNE" launch

scenario "Claude profile turns agent teams off"
comment "The --settings object carries only env, so it reaches the tool unchanged and nothing is dropped."
run "$RUNE run teams-off@claude --repo . 'hello' 2>&1"
expect "argv: .*--settings \{\"env\":\{\"CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS\":\"0\"\}\}"
expect_not "warning"
expect_not "owns these profile arguments"

scenario "Profile sets a permission mode and a permission grant"
comment "The run owns --permission-mode and the settings key permissions: both are dropped with a warning, env stays, the run continues."
run "$RUNE run widening@claude --repo . 'hello' 2>&1"
expect "warning: automated claude execution owns --permission-mode; the profile value acceptEdits is dropped"
expect "warning: automated claude execution owns settings key permissions"
expect "argv: .*--permission-mode plan"
expect "argv: .*--settings \{\"env\":\{\"X\":\"1\"\}\}"
expect_not "argv: .*acceptEdits"
expect_not "argv: .*permissions"

scenario "Profile carries an unowned flag"
comment "A flag the run does not set passes through untouched."
run "$RUNE run plain@claude --repo . 'hello' 2>&1"
expect "argv: .*--verbose"
expect_not "warning"
