# Automated Tool Run Design

## Approach

`rune run` consumes the same resolved launch plan as `rune launch`, then executes through the provider layer shared with native bench. This keeps profile, model, middleware, and preflight behavior in one resolver while preserving separate interactive and supervised process backends.

## Structure

- `rune run [profile@]<tool>` accepts one prompt from a positional argument, `--prompt-file`, or noninteractive stdin.
- `--repo` resolves to an existing canonical directory. `--mode` selects `read-only` or `workspace-write`, with read-only as the default.
- `--timeout` accepts milliseconds, seconds, minutes, hours, or bare seconds. Absence means no deadline.
- The resolved launch plan contributes the binary, profile arguments, environment, model route, warnings, and preflight checks.
- tmux and Docker wrappers fail before preflight or process creation.
- Claude, Codex, agy, Grok, and OpenCode requests use the provider adapters shared with bench.
- The process supervisor captures bounded output, drains both output streams concurrently, forwards interruption, terminates the child process group on failure, and reaps the direct child.
- Standard output carries the final provider answer. Standard error carries provider diagnostics. Global JSON mode returns typed success and failure objects.
- agy receives the requested timeout through its native print option and a later supervisor deadline. Other providers receive only the optional supervisor deadline.

## Risks

- Provider profiles can add flags that conflict with the automated contract. Provider-owned options are rejected with a configuration error.
- Inherited wrapper automation can mutate repositories. Rune removes `HARNESS_AUTOMATED` from every provider child environment.
- A child can exceed capture memory or block on a full pipe. Capture is bounded while drainage continues until cleanup completes.
- A descendant can leave the supervised process group. Cleanup covers the direct child's process group and does not claim control over a new session or daemon.
