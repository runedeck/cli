# Behavior proof: cursor-agent-surface

`record.sh` holds one scene per scenario of the `cursor-agent-surface` delta specifications, in the grammar of
`docs/proofs/driver.sh`. `proof.cast` is its recording against the built binary on 2026-10-02, exit 0. `proof.txt`
is the transcript `asciinema convert -f txt` wrote from it, and `proof.gif` the render.

Transcript digest (`shasum -a 256 proof.txt`): `9018c6039e3955caf8ff2e91410cbab4b2867abee96fbf9b0ac1b5ae400290fb`

The scenes build their own fixture off camera: a scratch `HOME` with four cursor profiles (`max` with a
configured `fable-max` route, `wide` with `--yolo --sandbox disabled`, `tools` with `--allowed-tools=shell`, `shell`
with the positional `install-shell-integration`), a second `HOME` whose config sets `tools.cursor.binary`, a Cursor
config directory with `approvalMode: unrestricted`, and a fake `cursor-agent` on `PATH`. The fake prints one Cursor
result object whose text names the arguments it received, the last line of its standard input, its `HOME` and
`CURSOR_CONFIG_DIR`, and whether `CURSOR_API_KEY` reached it. `FAKE_CURSOR_ERROR` and `FAKE_CURSOR_SANDBOX` make it
fail the way Cursor reports an error and the way it fails inside the harness sandbox. No model is called. The config
defines no `fable` profile, so `fable@cursor` shows the built-in one.

Re-record after a change to the scenes or the command:

```sh
RUNE=target/debug/rune asciinema rec --command "bash docs/proofs/cursor-agent-surface/record.sh" \
    --headless --window-size 100x30 --idle-time-limit 2 --overwrite --return proof.cast
agg --theme github-dark --font-size 12 --fps-cap 4 --last-frame-duration 4 proof.cast proof.gif
asciinema convert -f txt --overwrite proof.cast proof.txt
```

Where asciinema cannot open a pseudo terminal, `docs/proofs/cast.py` writes the same cast from the script's output stream:

```sh
RUNE=target/debug/rune python3 docs/proofs/cast.py docs/proofs/cursor-agent-surface/record.sh proof.cast
```
