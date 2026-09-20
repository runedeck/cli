#!/usr/bin/env bash
# Driver for a recorded acceptance proof. asciinema records this script.
# The grammar lives in ../driver.sh; edit only the scenes at the end.
# shellcheck source=../driver.sh
. "$(dirname "$0")/../driver.sh"

printf '\033[2J\033[H'

# Off camera: a stub model endpoint on a free local port and a scratch
# HOME whose rune config points two claude profiles at it. `kimi` sends
# the token the stub accepts. `stale` sends one it refuses. RUNE names the
# binary under test. Nothing outside the scratch directory changes.
RUNE=${RUNE:-rune}
PROOF_HOME=$(mktemp -d)
export HOME="$PROOF_HOME"
export CLIPROXY_API_KEY="proof-token"
mkdir -p "$PROOF_HOME/.config/rune"
cat > "$PROOF_HOME/stub.py" <<'PY'
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

MODELS = {"data": [{"id": "kimi-k3"}, {"id": "kimi-k2.7-code-highspeed"}, {"id": "gpt-5.6-sol"}]}

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass
    def do_GET(self):
        token = self.headers.get("Authorization", "")
        if self.path != "/v1/models":
            body, status = b'{"error":"not found"}', 404
        elif token == "Bearer proof-token":
            body, status = json.dumps(MODELS).encode(), 200
        else:
            body, status = b'{"error":"Invalid API key"}', 401
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

server = HTTPServer(("127.0.0.1", 0), Handler)
print(server.server_address[1], flush=True)
server.serve_forever()
PY
python3 "$PROOF_HOME/stub.py" > "$PROOF_HOME/port" 2>/dev/null &
STUB_PID=$!
trap 'kill "$STUB_PID" 2>/dev/null' EXIT
until [ -s "$PROOF_HOME/port" ]; do sleep 0.1; done
PORT=$(<"$PROOF_HOME/port")
cat > "$PROOF_HOME/.config/rune/config.yaml" <<YAML
launch:
  profiles:
    claude:
      kimi:
        model: kimi
        env:
          ANTHROPIC_BASE_URL: "http://127.0.0.1:$PORT"
          ANTHROPIC_AUTH_TOKEN:
            from_env: CLIPROXY_API_KEY
          ANTHROPIC_SMALL_FAST_MODEL: "kimi-k2.7-code-highspeed"
      stale:
        model: kimi
        env:
          ANTHROPIC_BASE_URL: "http://127.0.0.1:$PORT"
          ANTHROPIC_AUTH_TOKEN: "expired-token"
YAML
cd "$PROOF_HOME" || exit 1
preflight "$RUNE" launch

scenario "Profile names the kimi route"
comment "kimi is a built-in route: kimi-k3 on the Standard tier, overridable by models.kimi in config."
run "$RUNE launch kimi@claude --dry-run"
expect "model_id: kimi-k3"
expect "model_context: 262144"
expect "model_source: built-in"

scenario "Route model and small fast model are served"
comment "--check asks the plan's base URL for /v1/models with the plan's own credential. No spawn."
run "$RUNE launch kimi@claude --check; echo \"exit \$?\""
expect "credential: ANTHROPIC_AUTH_TOKEN"
expect "served +kimi-k3"
expect "served +kimi-k2.7-code-highspeed"
expect "^exit 0$"
expect_not "proof-token"

scenario "Run check with a model override"
comment "rune run shares the check. --model replaces the route id, and the prompt is never read."
run "$RUNE run kimi@claude --check --model kimi-k4; echo \"exit \$?\""
expect "missing +kimi-k4"
expect "served +kimi-k2.7-code-highspeed"
expect "^exit 1$"

scenario "Endpoint refuses the credential"
comment "A refusal is an endpoint failure, exit 2, never a missing model. The token stays out of the report."
run "$RUNE launch stale@claude --check; echo \"exit \$?\""
expect "HTTP 401 \(credential refused\)"
expect "^exit 2$"
expect_not "missing"
expect_not "expired-token"

scenario "Plan carries no base URL"
comment "A bare tool with no profile has nothing to check."
run "$RUNE launch claude --check; echo \"exit \$?\""
expect "nothing to check"
expect "^exit 0$"
