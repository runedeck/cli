"""Run a command against an HTTP proxy that accepts and never answers.

The proxy listens on a free local port and holds every connection open
without a reply, so a client that tunnels through it sees a feed that stays
silent. The command runs with ALL_PROXY pointing at the proxy, its output
passes through, and the proxy then prints each request line it received
and the whole seconds the command took. The exit status is the command's.

    python3 silent-proxy.py rune discover
"""

import os
import socket
import subprocess
import sys
import threading
import time

requests = []
held = []


def serve(listener):
    while True:
        try:
            connection, _ = listener.accept()
        except OSError:
            return
        held.append(connection)
        line = connection.makefile("rb").readline().decode("latin-1").strip()
        requests.append(line)


listener = socket.socket()
listener.bind(("127.0.0.1", 0))
listener.listen()
port = listener.getsockname()[1]
threading.Thread(target=serve, args=(listener,), daemon=True).start()

environment = dict(os.environ, ALL_PROXY=f"http://127.0.0.1:{port}")
started = time.monotonic()
status = subprocess.call(sys.argv[1:], env=environment)
elapsed = time.monotonic() - started

for line in requests:
    print(f"proxy received: {line}")
print("proxy answered: nothing")
print(f"command returned after: {int(elapsed)} seconds")
sys.exit(status)
