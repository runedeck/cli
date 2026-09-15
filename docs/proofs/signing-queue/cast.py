#!/usr/bin/env python3
"""Capture record.sh as an asciinema v2 cast without a pty.

asciinema needs a pseudo terminal, which a sandbox may refuse. This reads
the script's combined output in chunks, stamps each chunk with the time
since start, and writes the same event stream asciinema would.

    RUNE=target/debug/rune python3 cast.py proof.cast
"""

import json
import os
import subprocess
import sys
import time

WIDTH, HEIGHT = 100, 30


def main() -> int:
    target = sys.argv[1] if len(sys.argv) > 1 else "proof.cast"
    script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "record.sh")
    env = dict(os.environ, FORCE_COLOR="1", COLUMNS=str(WIDTH), LINES=str(HEIGHT))
    start = time.monotonic()
    header = {
        "version": 2,
        "width": WIDTH,
        "height": HEIGHT,
        "timestamp": int(time.time()),
        "env": {"SHELL": "/bin/bash", "TERM": "xterm-256color"},
    }
    with open(target, "w", encoding="utf-8") as cast:
        cast.write(json.dumps(header) + "\n")
        with subprocess.Popen(
            ["bash", script],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            env=env,
        ) as process:
            assert process.stdout is not None
            while chunk := process.stdout.read1(4096):
                text = chunk.decode("utf-8", "replace").replace("\n", "\r\n")
                cast.write(json.dumps([round(time.monotonic() - start, 3), "o", text]) + "\n")
            status = process.wait()
    if status != 0:
        print(f"record.sh exited {status}", file=sys.stderr)
    return status


if __name__ == "__main__":
    sys.exit(main())
