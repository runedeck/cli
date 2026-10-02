#!/usr/bin/env bash
# Driver for a recorded acceptance proof. asciinema records this script.
# The grammar lives in ../driver.sh; edit only the scenes at the end.
# shellcheck source=../driver.sh
. "$(dirname "$0")/../driver.sh"

printf '\033[2J\033[H'

# Off camera: a scratch HOME whose rune config carries the cursor
# profiles max, wide, tools, and shell, a Cursor config directory with
# approvalMode unrestricted, and a fake `cursor-agent` on PATH. The fake
# prints one Cursor result object whose text names the arguments it
# received, the last line of its standard input, and its HOME and
# CURSOR_CONFIG_DIR. FAKE_CURSOR_ERROR makes it report an error,
# FAKE_CURSOR_SANDBOX makes it fail the way cursor-agent fails inside the
# harness sandbox. The fake prints the scratch HOME as `~`, as a shell
# prompt does. No model is called. RUNE names the binary under test.
RUNE=${RUNE:-rune}
unset SANDBOX_RUNTIME CURSOR_API_KEY CURSOR_CONFIG_DIR XDG_CONFIG_HOME
PROOF_HOME=$(cd "$(mktemp -d)" && pwd -P)
BINARY_HOME=$(mktemp -d)
UNRESTRICTED_DIR=$(mktemp -d)
export HOME="$PROOF_HOME" FAKE_CURSOR_HOME="$PROOF_HOME"
mkdir -p "$PROOF_HOME/.config/rune" "$PROOF_HOME/bin" "$PROOF_HOME/repo" "$BINARY_HOME/.config/rune"
cat > "$PROOF_HOME/bin/cursor-agent" <<'SH'
#!/usr/bin/env bash
# Fake cursor-agent: answer with the argv, the prompt, and the homes.
prompt=$(tail -n 1)
if [ -n "${FAKE_CURSOR_SANDBOX:-}" ]; then
    for _ in 1 2 3 4 5 6 7 8; do
        echo "ERROR: failed to copy trust settings of system certificate-25291" >&2
    done
    echo "Error: Authentication required. Please run 'agent login' first." >&2
    exit 1
fi
if [ -n "${FAKE_CURSOR_ERROR:-}" ]; then
    jq -cn --arg r "$FAKE_CURSOR_ERROR" '{type:"result",subtype:"error",is_error:true,result:$r}'
    exit 0
fi
key=no
[ -n "${CURSOR_API_KEY:-}" ] && key=yes
text="argv: $* | stdin: $prompt | HOME=$HOME CURSOR_CONFIG_DIR=${CURSOR_CONFIG_DIR:-unset} key-in-env=$key"
text=${text//$FAKE_CURSOR_HOME/\~}
jq -cn --arg r "$text" '{type:"result",subtype:"success",is_error:false,result:$r,usage:{inputTokens:12,outputTokens:3}}'
SH
chmod +x "$PROOF_HOME/bin/cursor-agent"
export PATH="$PROOF_HOME/bin:$PATH"
cat > "$PROOF_HOME/.config/rune/config.yaml" <<'YAML'
launch:
  models:
    fable-max:
      id: claude-fable-5-1-max
      context: 1000000
  profiles:
    cursor:
      max:
        model: fable-max
      wide:
        args:
          - "--yolo"
          - "--sandbox"
          - "disabled"
      tools:
        args:
          - "--allowed-tools=shell"
      shell:
        args:
          - "install-shell-integration"
YAML
cat > "$BINARY_HOME/.config/rune/config.yaml" <<'YAML'
launch:
  tools:
    cursor:
      binary: /opt/cursor/bin/agent
YAML
printf '{"version":1,"approvalMode":"unrestricted"}\n' > "$UNRESTRICTED_DIR/cli-config.json"
cd "$PROOF_HOME/repo" || exit 1
preflight "$RUNE" launch

scenario "Cursor launch without a configured binary"
comment "The cursor tool starts cursor-agent, because the cursor command on a Mac opens the editor."
run "$RUNE launch cursor --dry-run"
expect "cursor-agent"
run "$RUNE launch"
expect "cursor"

scenario "Cursor launch with a configured binary"
comment "A second config sets tools.cursor.binary to /opt/cursor/bin/agent."
run "$RUNE launch cursor --dry-run" "HOME=$BINARY_HOME $RUNE launch cursor --dry-run"
expect "/opt/cursor/bin/agent"

scenario "Cursor run with a profile model"
comment "The profile model becomes --model, and the prompt travels on standard input."
run "$RUNE run max@cursor 'Reply with pong'"
expect "argv: -p --output-format json --workspace [^ ]+ --trust --mode ask --model claude-fable-5-1-max \|"
expect "stdin: Reply with pong"
expect_not "argv: [^|]*Reply with pong"

scenario "Built-in Fable profile for Cursor"
comment "The config defines no fable profile for cursor: the built-in one selects claude-fable-5-1-high."
run "$RUNE run fable@cursor 'Reply with pong'"
expect "argv: .*--model claude-fable-5-1-high \|"
expect_not "warning"
run "$RUNE launch fable@cursor --dry-run"
expect "argv: cursor-agent --model claude-fable-5-1-high"
run "$RUNE launch"
expect "fable@cursor"

scenario "Read-only run asks for an edit"
comment "Read-only is the default: ask mode, and no --force, --yolo, or --sandbox."
run "$RUNE run cursor 'Create hello.txt'"
expect "argv: .*--mode ask"
expect_not "argv: [^|]*(--force|--yolo|--sandbox)"

scenario "Read-only run under unrestricted approval"
comment "Defense in depth: a saved approvalMode unrestricted is the one mode that would run a write unasked."
run "$RUNE --json run cursor 'hello'; echo \"exit \$?\"" "CURSOR_CONFIG_DIR=$UNRESTRICTED_DIR $RUNE --json run cursor 'hello'; echo \"exit \$?\""
expect "configuration_error"
expect "approvalMode unrestricted"
expect "exit 2"
expect_not "argv:"

scenario "Workspace-write run creates a file"
comment "Workspace-write lets Cursor apply edits inside its own sandbox."
run "$RUNE run cursor --mode workspace-write 'Create hello.txt'"
expect "argv: .*--force --sandbox enabled"
expect_not "argv: [^|]*--mode ask"

scenario "Cursor reports an error in its result"
comment "Cursor exits 0 but sets is_error: the run fails with the tool's message."
run "$RUNE --json run cursor 'hello'; echo \"exit \$?\"" "FAKE_CURSOR_ERROR='rate limited' $RUNE --json run cursor 'hello'; echo \"exit \$?\""
expect "rate limited"
expect "exit 1"
expect_not "\"ok\": ?true"

scenario "Clean run without a key"
comment "The browser login lives in the Keychain, which a clean home cannot reach."
run "$RUNE --json run cursor --clean 'Reply with pong'; echo \"exit \$?\""
expect "configuration_error"
expect "CURSOR_API_KEY"
expect "Keychain"
expect "exit 2"
expect_not "argv:"

scenario "Clean run with a key"
comment "HOME and CURSOR_CONFIG_DIR move into the clean root. The key stays in the environment."
run "CURSOR_API_KEY=proof-key $RUNE run cursor --clean 'Reply with pong'"
expect "CURSOR_CONFIG_DIR=[^ ]+/\.cursor"
expect "key-in-env=yes"
expect_not "HOME=~ "
expect_not "argv: [^|]*proof-key"

scenario "Cursor run inside the harness sandbox"
comment "cursor-agent cannot read the Keychain in the sandbox: the failure keeps its stderr and adds a hint."
run "$RUNE run cursor 'hello'" "FAKE_CURSOR_SANDBOX=1 $RUNE run cursor 'hello'"
expect "hint: cursor-agent cannot read its Keychain login"
expect "failed to copy trust settings"

scenario "Cursor profile widens access"
comment "The profile carries --yolo and --sandbox disabled: both are dropped with a warning each."
run "$RUNE run wide@cursor 'hello'"
expect "warning: .*--yolo"
expect "warning: .*--sandbox.*disabled"
expect "argv: .*--mode ask"
expect_not "argv: [^|]*(--yolo|disabled)"

scenario "Cursor profile carries an unlisted flag"
comment "cursor-agent accepts --allowed-tools although its help does not list it. The run keeps only --endpoint."
run "$RUNE run tools@cursor 'hello'"
expect "warning: .*--allowed-tools"
expect_not "warning: .*shell"
expect_not "argv: [^|]*--allowed-tools"

scenario "Cursor profile names a subcommand"
comment "The profile carries install-shell-integration, which cursor-agent would run as a subcommand."
run "$RUNE run shell@cursor 'hello'"
expect "warning: .*positional profile argument"
expect "stdin: hello"
expect_not "argv: [^|]*install-shell-integration"
