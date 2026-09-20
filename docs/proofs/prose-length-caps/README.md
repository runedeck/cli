# Behavior proof: prose-length-caps

`record.sh` holds one scene per scenario of the `prose-length-caps` delta specification, in the grammar of
`docs/proofs/driver.sh`. `proof.cast` is its recording against the built binary on 2026-09-20, exit 0.
`proof.txt` is the transcript `asciinema convert -f txt` wrote from it, and `proof.gif` the render.

Transcript digest (`shasum -a 256 proof.txt`): `8970469ae14a2ac48a40d91d5366e42a4b29741eecee7b1a6eff09f473e9c41c`

The scenes build a scratch repository with one canonical specification, one change, and a clean
`CHANGELOG.md`, then break one file per scene and run the checker on it.

Re-record after a change to the scenes or the command:

```sh
RUNE=target/debug/rune asciinema rec --command "bash docs/proofs/prose-length-caps/record.sh" \
    --headless --window-size 100x30 --idle-time-limit 2 --overwrite --return proof.cast
agg --theme github-dark --font-size 11 --fps-cap 3 --last-frame-duration 3 proof.cast proof.gif
asciinema convert -f txt --overwrite proof.cast proof.txt
```

Where asciinema cannot open a pseudo terminal, `docs/proofs/cast.py` writes the same cast from the script's output stream:

```sh
RUNE=target/debug/rune python3 docs/proofs/cast.py docs/proofs/prose-length-caps/record.sh proof.cast
```
