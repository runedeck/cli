#!/usr/bin/env bash
# The recorded-proof driver grammar: scenario, run, expect, expect_not,
# preflight. A proof's record.sh sources this file and holds only its
# scenes. A missed expectation exits nonzero, and the recorder carries
# that status out. The variables at the top tune pace; a proof exports
# them before sourcing when it needs another pace.
set -u

TYPE_DELAY=${TYPE_DELAY:-0.02}
PAUSE_AFTER_COMMENT=${PAUSE_AFTER_COMMENT:-1.2}
PAUSE_AFTER_OUTPUT=${PAUSE_AFTER_OUTPUT:-1.5}

PROMPT_COLOR=$'\033[1;32m'
COMMENT_COLOR=$'\033[2m'
FAIL_COLOR=$'\033[1;31m'
RESET=$'\033[0m'

LAST_OUTPUT=""
CURRENT_SCENARIO=""
SCENE_HAS_RUN=0
OUTPUT_FILE=$(mktemp)
# On a failed run the captured output of the last scene is the diagnosis.
trap 'status=$?; [ "$status" -ne 0 ] && sed "s/^/last output: /" "$OUTPUT_FILE" >&2; rm -f "$OUTPUT_FILE"' EXIT

prompt() {
    printf '%s❯%s ' "$PROMPT_COLOR" "$RESET"
}

type_out() {
    local text=$1
    local index
    for ((index = 0; index < ${#text}; index++)); do
        printf '%s' "${text:index:1}"
        sleep "$TYPE_DELAY"
    done
}

comment() {
    printf '%s# %s%s\n' "$COMMENT_COLOR" "$1" "$RESET"
    sleep "$PAUSE_AFTER_COMMENT"
}

# scenario TITLE
# Starts a scene named for the specification scenario it proves. The
# captured output is cleared, so an expect before the scene's first run
# fails instead of matching the previous scene.
scenario() {
    CURRENT_SCENARIO=$1
    LAST_OUTPUT=""
    SCENE_HAS_RUN=0
    printf '\n%s# Scenario: %s%s\n' "$COMMENT_COLOR" "$1" "$RESET"
    sleep "$PAUSE_AFTER_COMMENT"
}

# run SHOWN [ACTUAL]
# SHOWN is the command the viewer sees. ACTUAL is the command that runs.
# ACTUAL defaults to SHOWN. Use ACTUAL to filter tool noise off camera.
# The command runs in this shell, not a subshell, so cd and export carry
# into the next run. The output is printed and kept for the next expect.
run() {
    # The viewer sees the command as the owner types it: `rune`, not the
    # path of the binary under test.
    local shown=${1//$RUNE/rune}
    local actual=${2:-$1}
    prompt
    type_out "$shown"
    printf '\n'
    eval "$actual" >"$OUTPUT_FILE" 2>&1
    LAST_OUTPUT=$(<"$OUTPUT_FILE")
    SCENE_HAS_RUN=1
    printf '%s\n' "$LAST_OUTPUT"
    sleep "$PAUSE_AFTER_OUTPUT"
}

# matches PATTERN
# Returns 0 on a match and 1 on no match. An expectation before the
# scene's first run, or a malformed pattern (matcher status 2), is an
# unevaluated expectation, and that must never pass, so both exit 1.
matches() {
    local status
    if [ "$SCENE_HAS_RUN" -eq 0 ]; then
        printf '%sexpectation before any run in scenario "%s": /%s/%s\n' "$FAIL_COLOR" "$CURRENT_SCENARIO" "$1" "$RESET"
        exit 1
    fi
    printf '%s\n' "$LAST_OUTPUT" | grep -Eq -- "$1"
    status=$?
    if [ "$status" -gt 1 ]; then
        printf '%smalformed pattern in scenario "%s": /%s/%s\n' "$FAIL_COLOR" "$CURRENT_SCENARIO" "$1" "$RESET"
        exit 1
    fi
    return "$status"
}

# expect PATTERN
# Matches an extended regular expression against the last run's output.
# A miss prints the pattern and the output, then exits 1.
expect() {
    if matches "$1"; then
        return 0
    fi
    printf '%sexpectation failed in scenario "%s": /%s/%s\n' "$FAIL_COLOR" "$CURRENT_SCENARIO" "$1" "$RESET"
    printf '%s\n' "$LAST_OUTPUT"
    exit 1
}

# expect_not PATTERN
# The inverse: exits 1 when the pattern appears.
expect_not() {
    if matches "$1"; then
        printf '%sunexpected output in scenario "%s": /%s/%s\n' "$FAIL_COLOR" "$CURRENT_SCENARIO" "$1" "$RESET"
        printf '%s\n' "$LAST_OUTPUT"
        exit 1
    fi
}

# preflight COMMAND...
# Runs a command once with its output hidden, so a keychain or permission
# dialog is answered before the first scene.
preflight() {
    "$@" >/dev/null 2>&1 || true
}

