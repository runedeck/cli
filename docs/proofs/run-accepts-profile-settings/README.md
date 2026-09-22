# Behavior proof: run-accepts-profile-settings

`record.sh` holds one scene per scenario of the `run-accepts-profile-settings` delta specification, in the
grammar of `docs/proofs/driver.sh`. `proof.cast` is its recording against the built binary on 2026-09-22,
exit 0. `proof.txt` is the transcript `asciinema convert -f txt` wrote from it, and `proof.gif` the render.

Transcript digest (`shasum -a 256 proof.txt`): `4fa2b95dfed639efdc825bc5a1d092d9c5549276e5e128a9aeb72830fc3bc9ae`

The scenes build their own fixture off camera: a scratch `HOME` with three claude profiles (`teams-off`
with an `env`-only `--settings`, `widening` with `--permission-mode acceptEdits` and a `permissions` grant
inside `--settings`, `plain` with `--verbose`) and a fake `claude` on `PATH` that prints the arguments it
received. No model is called. The `argv:` line in each scene is what reached the tool.

Re-record after a change to the scenes or the command:

```sh
RUNE=target/debug/rune asciinema rec --command "bash docs/proofs/run-accepts-profile-settings/record.sh" \
    --headless --window-size 100x30 --idle-time-limit 2 --overwrite --return proof.cast
agg --theme github-dark --font-size 12 --fps-cap 4 --last-frame-duration 4 proof.cast proof.gif
asciinema convert -f txt --overwrite proof.cast proof.txt
```

Where asciinema cannot open a pseudo terminal, `docs/proofs/cast.py` writes the same cast from the script's output stream:

```sh
RUNE=target/debug/rune python3 docs/proofs/cast.py docs/proofs/run-accepts-profile-settings/record.sh proof.cast
```
