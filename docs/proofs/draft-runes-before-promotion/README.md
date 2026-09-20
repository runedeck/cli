# Behavior proof: draft-runes-before-promotion

`record.sh` holds one scene per scenario of the `draft-runes-before-promotion` delta specification, in the
grammar of `docs/proofs/driver.sh`. `proof.cast` is its recording against the built binary on 2026-09-20,
exit 0. `proof.txt` is the transcript `asciinema convert -f txt` wrote from it, and `proof.gif` the render.

Transcript digest (`shasum -a 256 proof.txt`): `36d3b523dd534757adc23ccd73eb0194af13904f28ccae6c99d786fe798de994`

The scenes build their own fixture off camera: a scratch deck with the `core` domain and the RTK skill copied
from `DECK_SOURCE` (default `../deck` beside this repository), and a consumer whose `.rune` names it, deployed
with `rune install`. Nothing outside the scratch directory changes.

Re-record after a change to the scenes or the command:

```sh
RUNE=target/debug/rune asciinema rec --command "bash docs/proofs/draft-runes-before-promotion/record.sh" \
    --headless --window-size 100x30 --idle-time-limit 2 --overwrite --return proof.cast
agg --theme github-dark --font-size 12 --fps-cap 4 --last-frame-duration 4 proof.cast proof.gif
asciinema convert -f txt --overwrite proof.cast proof.txt
```

Where asciinema cannot open a pseudo terminal, `docs/proofs/cast.py` writes the same cast from the script's output stream:

```sh
RUNE=target/debug/rune python3 docs/proofs/cast.py docs/proofs/draft-runes-before-promotion/record.sh proof.cast
```
