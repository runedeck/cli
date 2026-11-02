#!/usr/bin/env python3
"""A System One stub for the ask proof: one local HTTP server that
answers `POST /v1/systemone` with fixed, typed answers and `GET /v1/models`
with a fixed model list, and records every request it saw, with the reply
it gave, as one JSON line, so a scene can show what left the machine and
what came back.

The stub binds 127.0.0.1 on a free port, writes the port to the file
`--port-file` names, and exits by itself after thirty seconds without a
request, so a scene that stops early leaves no server behind.

Options:
  --status N        the status of the systemone reply (default 200)
  --echo-key        the reply body quotes the bearer token, as a refusal would
  --delay S         seconds to wait before answering systemone
  --models LIST     comma-separated ids `GET /v1/models` lists (default jev-1.13.0)
  --record FILE     where each request goes as one JSON line
  --port-file FILE  where the bound port is written
"""

import argparse
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

# The answers come back out of key order on purpose: the command sorts
# them by question key when it prints.
ANSWERS = {
    "model": "jev-1.13.0",
    "answers": {
        "shape": {"type": "choice", "choice": "one sitting"},
        "mood": {"type": "noul", "noul": 0.83},
    },
    "usage": {"state_tokens": 12, "question_tokens": 9},
}

IDLE_SECONDS = 30


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--status", type=int, default=200)
    parser.add_argument("--echo-key", action="store_true")
    parser.add_argument("--delay", type=float, default=0.0)
    parser.add_argument("--models", default="jev-1.13.0")
    parser.add_argument("--record", required=True)
    parser.add_argument("--port-file", required=True)
    options = parser.parse_args()

    # An empty record from the start, so a scene can show zero requests.
    open(options.record, "w").close()

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *_):
            pass

        def reply(self, body, status, payload):
            """Record the request and the reply as one line, then send
            the reply. The line is written before any delay, so a scene
            that gives up on the request still sees it recorded."""
            try:
                parsed = json.loads(body) if body else ""
            except ValueError:
                parsed = body
            line = {
                "method": self.command,
                "path": self.path,
                "authorization": self.headers.get("Authorization"),
                "body": parsed,
                "status": status,
                "reply": payload,
            }
            with open(options.record, "a") as record:
                record.write(json.dumps(line) + "\n")
            if options.delay and self.command == "POST":
                time.sleep(options.delay)
            data = json.dumps(payload).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if self.path.endswith("/models"):
                ids = [{"id": name} for name in options.models.split(",")]
                self.reply("", 200, {"object": "list", "data": ids})
            else:
                self.reply("", 404, {"error": "no such route"})

        def do_POST(self):
            length = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(length).decode()
            if not self.path.endswith("/systemone"):
                self.reply(body, 404, {"error": "no such route"})
            elif options.echo_key:
                token = (self.headers.get("Authorization") or "").removeprefix("Bearer ")
                self.reply(body, options.status, {"error": "invalid api key: " + token})
            elif options.status == 200:
                self.reply(body, 200, ANSWERS)
            else:
                self.reply(body, options.status, {"error": "service unavailable"})

    server = HTTPServer(("127.0.0.1", 0), Handler)
    server.timeout = IDLE_SECONDS
    with open(options.port_file, "w") as port_file:
        port_file.write(str(server.server_address[1]))
    while True:
        # handle_request returns without serving when the idle timeout
        # passes; the stub then leaves on its own.
        before = time.monotonic()
        server.handle_request()
        if time.monotonic() - before >= IDLE_SECONDS:
            return 0


if __name__ == "__main__":
    sys.exit(main())
