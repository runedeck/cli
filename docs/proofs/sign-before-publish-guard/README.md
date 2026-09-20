# Behavior proof: sign-before-publish-guard

`record.sh` holds one scene per scenario of the `sign-before-publish-guard` delta specification, in the
grammar of `docs/proofs/driver.sh` on the fixture of `docs/proofs/signing-fixture.sh`. `proof.cast` is its
recording against the built binary on 2026-09-20, exit 0. `proof.txt` is the transcript
`asciinema convert -f txt` wrote from it, and `proof.gif` the render.

Transcript digest (`shasum -a 256 proof.txt`): `47fe5a5d443ca495425d8d4fe161509dfeb0a6cc7f4be3bb2cf4c5128edbaa88`

The fixture's `origin` is a bare repository beside the clone with `main` at the base. The guard reads the
remote-tracking ref alone, so a scene publishes a head with `git update-ref`, as a fetch would leave it.

Re-record after a change to the scenes or the command:

```sh
RUNE=target/debug/rune asciinema rec --command "bash docs/proofs/sign-before-publish-guard/record.sh" \
    --headless --window-size 100x30 --idle-time-limit 2 --overwrite --return proof.cast
agg --theme github-dark --font-size 11 --fps-cap 3 --last-frame-duration 3 proof.cast proof.gif
asciinema convert -f txt --overwrite proof.cast proof.txt
```

Where asciinema cannot open a pseudo terminal, `docs/proofs/cast.py` writes the same cast from the script's output stream:

```sh
RUNE=target/debug/rune python3 docs/proofs/cast.py docs/proofs/sign-before-publish-guard/record.sh proof.cast
```
