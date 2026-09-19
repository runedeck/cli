# Behavior proof: signing-queue

`record.sh` holds one scene per scenario of the `signing-queue` delta specification, in the AcceptanceTesting
driver's grammar. `proof.cast` is its recording against the built binary on 2026-09-15, exit 0. `proof.txt` is the
transcript `asciinema convert -f txt` wrote from it, and `proof.gif` the render.

Transcript digest (`shasum -a 256 proof.txt`): `83f9c0d5c7f764a54c93ac5bd9f126ac6f4152ef4f7917ca8b1a4559e8ffdaf0`

Re-record after a change to the scenes or the command:

```sh
RUNE=target/debug/rune asciinema rec --command "bash docs/proofs/signing-queue/record.sh" \
    --headless --window-size 100x30 --idle-time-limit 2 --overwrite --return proof.cast
agg --theme github-dark --font-size 14 --fps-cap 8 --last-frame-duration 4 proof.cast proof.gif
asciinema convert -f txt --overwrite proof.cast proof.txt
```

Where asciinema cannot open a pseudo terminal, `cast.py` writes the same cast from the script's output stream:

```sh
RUNE=target/debug/rune python3 docs/proofs/signing-queue/cast.py proof.cast
```
