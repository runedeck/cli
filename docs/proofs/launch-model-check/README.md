# Behavior proof: launch-model-check

`record.sh` holds one scene per scenario of the `launch-model-check` delta specifications, in the
grammar of `docs/proofs/driver.sh`. `proof.cast` is its recording against the built binary on 2026-09-20,
exit 0. `proof.txt` is the transcript `asciinema convert -f txt` wrote from it, and `proof.gif` the render.

Transcript digest (`shasum -a 256 proof.txt`): `0f2e36330f8a8014d86d1c4c8d7b0e028cfa863e57bf7420437170aed3d6cd61`

The scenes build their own fixture off camera: a stub `/v1/models` endpoint on a free local port that lists
`kimi-k3`, `kimi-k2.7-code-highspeed`, and `gpt-5.6-sol` for one bearer token and answers 401 to any other, and
a scratch `HOME` whose rune config points a `kimi` and a `stale` claude profile at it. Nothing outside the scratch
directory changes, and no real endpoint is called.

## Live run

The same binary against the owner's local proxy on 2026-09-20, plain HTTP listener, with the owner's credential
in the environment. Recorded here because the stub cannot prove that a real proxy lists the route:

```text
❯ rune launch kimi@claude --check
tool: claude
endpoint: http://127.0.0.1:8317/v1/models
credential: ANTHROPIC_AUTH_TOKEN
  served   kimi-k3
  served   kimi-k2.7-code-highspeed
exit: 0

❯ rune run kimi@claude --check --model kimi-k4
  served   kimi-k2.7-code-highspeed
  missing  kimi-k4
exit: 1

❯ rune run kimi@claude "Reply with exactly two words: kimi ready"
kimi ready
```

The `https://` route to the same proxy failed inside the recording sandbox with `OSStatus -26276`, the sandbox
refusing keychain reads. The check uses the platform verifier, so outside the sandbox it trusts the same private
CA that curl and the harness trust.

Re-record after a change to the scenes or the command:

```sh
RUNE=target/debug/rune asciinema rec --command "bash docs/proofs/launch-model-check/record.sh" \
    --headless --window-size 100x30 --idle-time-limit 2 --overwrite --return proof.cast
agg --theme github-dark --font-size 12 --fps-cap 4 --last-frame-duration 4 proof.cast proof.gif
asciinema convert -f txt --overwrite proof.cast proof.txt
```

Where asciinema cannot open a pseudo terminal, `docs/proofs/cast.py` writes the same cast from the script's output stream:

```sh
RUNE=target/debug/rune python3 docs/proofs/cast.py docs/proofs/launch-model-check/record.sh proof.cast
```
