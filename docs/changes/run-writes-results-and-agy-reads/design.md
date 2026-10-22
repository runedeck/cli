# Run writes results and agy reads: design

## Approach

`--output <file>` beat telling callers to read a background task's log. A caller in a Claude Code session can do that, but a hook, a plugin, or a script cannot, and every one of them reaches for a redirect. The runner already builds the result as one value for `--json`, so writing it to a file is the same printer with another destination.

An output flag is also an arbitrary file write by any session that may call rune without a prompt. The runner therefore checks the path before the harness starts. The file must sit inside the repository the caller works in, the system temporary directory (`std::env::temp_dir()` and `/tmp`, both canonicalized), or the rune state directory. The repository root is the jj or git root above the caller's working directory, never the folder `--repo` names: with `--repo ~/.claude` the old rule accepted `~/.claude/settings.json`, and with `--repo ~` it accepted `~/.zshrc` (reviewer repro with the built binary). No directory between that root and the file may be hidden, the file name must not be hidden, the target must not be a symlink, and the parent must exist. The runner opens the checked parent folder and holds the handle until the write: the probe, the temporary file (`openat` with `O_CREAT | O_EXCL`, mode 0600), the sync, the rename over the target (`renameat`) and the cleanup (`unlinkat`) are all relative to that handle, through the nix crate's `fs` feature. A folder swapped for a symlink while the harness runs (reviewer repro `toctou.sh`) then cannot move the write: the handle names the folder the check accepted, or a folder that is gone, and the write fails with exit 74 instead of a write into the symlink's target. A parse error from clap happens before the path is accepted, so it prints on stdout and writes no file.

Under `--json`, stdout carries `{"ok":…,"kind":…,"exit_code":…,"output":"<abs path>"}`. When the final write fails, stdout carries the full result, and the exit code is 74 (`EX_IOERR`), so the answer is never lost.

For agy, the experiments of 2026-10-22 settled the read-only question. agy wrote files in `--mode plan` with the owner's policy, and it wrote them with a clean settings file that denied `write_file(*)`. The runner cannot make agy read-only, so it refuses read-only agy and points the caller at workspace-write mode, which runs in its own jj workspace. A workspace-write agy run gets an instruction in the prompt to read with its file tools.

agy writes its soft-denials only to its log. The runner passes `--log-file` with a path in the run's scratch directory, and reads it before it removes the scratch directory and before the clean state goes. The soft-deny lines travel as the failure's harness errors, and the hint for them is a function beside the network allowlist hint rather than a row of the static hint table, because the hint names the denied tool from the line itself.

The refusal of a mode a harness cannot take happens in the runner (`src/cli/run/mod.rs`), right after the harness is known and before a profile's pre-steps or any clean state: a configuration error with exit 2. The adapter module (`src/cli/surface/mod.rs`) keeps the same check as a guard in front of its argument builders. The answer-only refusal for codex, grok, and agy moved to the same place, so it is now exit 2 as well instead of an argument error with exit 1.

For grok, the runner parses the owner's `config.toml` and copies only `model.<id>.env_key` into the clean configuration. A file that does not parse is reported by line and column alone, computed from the toml crate's span: the crate's own message quotes the source line, which may hold a credential, so it never passes through. It never copies `telemetry`, `ui`, `sandbox`, `shell_environment_policy`, `session`, or `marketplace`. The dry run lists the kept key names as `clean_config_keys:` (the dotted TOML paths), and it prints neither the environment variable names those keys hold nor their values. The value of every variable an `env_key` entry names joins the run's secrets whatever the variable is called, so a key under a name that looks like no credential is still redacted from the stderr, the failure tails, the answer text, and the result file. The answer text is redacted for every harness now, because a model that echoes a key is a leak too.

The check sends only a credential the route names. For a tool outside the Anthropic and OpenAI families it takes the first credential-shaped key of the plan's own environment, else, from the inherited environment, a credential-shaped key under the tool's own name (`GROK_CODE_XAI_API_KEY`) or a variable that a grok `env_key` entry names. Any other credential in the shell, `AWS_SECRET_ACCESS_KEY` or a proxy token under its own name, is never sent (reviewer repro `check2.sh`): nothing the route carries names it, so the endpoint has no claim to it. A unit test with a fake AWS value proves that the stub endpoint receives no authorization header.

The `owns` warning for a dropped profile argument shows the value only for an option whose values come from a fixed list the harness documents (`--sandbox`, `--permission-mode`, `--output-format`, `--mode`, opencode's `--format`, codex's `sandbox_mode` and `approval_policy`), so the owner reads which mode was dropped. Every other value is hidden whole. A by-name redaction showed an MCP server's `args` array with `--api-key` and the key side by side (reviewer repro), because no name in that value looks like a credential, and no rule over names can know every shape a free value takes.

The output path check judges the hidden-folder rule from the closest permitted root (the repository when the repository sits under the temporary directory), and it probes the parent with an exclusive file it removes at once, so "the parent cannot take a new file" is tested rather than inferred from permission bits.

## Structure

- `src/cli/mod.rs`: the `--output <FILE>` flag on `rune run`.
- `src/cli/run/output.rs` (new): the path check and the atomic write.
- `src/cli/run/mod.rs`: the one result printer gains the file destination and the stdout confirmation.
- `src/cli/surface/mod.rs`: the read-only agy refusal, the prompt instruction, `--log-file`, the hint row, and the grok key filter.
- Tests: unit tests for the path check, the hint, and the grok filter. Integration tests in `tests/run.rs` with a fake agy and a fake grok.

## Commands

| Role | Command |
| --- | --- |
| A council seat writes its answer to a file | `rune run sol@codex --json --output /abs/seat.json --prompt-file /abs/brief.md` |
| The runner starts a workspace-write agy seat | `agy --sandbox --mode accept-edits --log-file <scratch>/agy.log --print <prompt> --output-format json --model <id>` |

## Risks

- A future agy may rename its soft-deny line or drop `--log-file`. The hint then stays silent, and a test row pins the current text.
- The prompt instruction is advice to a model. Isolation, not the instruction, protects the caller's checkout.
- `/tmp` is shared with other users on a multi-user machine. Mode 0600 and the symlink refusal limit what another user can do with it.
