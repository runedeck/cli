# Proofs share one format: spike

## scrut

scrut 0.4.3 was built from crates.io on 22 October 2026 with `cargo install --locked scrut --version 0.4.3` (binary sha256 `3f9205f7188b09b189393c4f0e822a3d4fd6509d4ab8558b0f01d87d0604665c`). Its `--version` prints a build timestamp, a bug that 0.5.0 fixes. A proof-shaped document ran under `scrut test` and showed:

- rune's frontmatter keys are ignored, and `defaults.output_stream: combined` applies to every block.
- `cd` and exported variables persist between blocks. A heredoc works in `>` continuation lines. Combined output keeps the order of stdout and stderr. `* (glob*)` matches zero or more lines. `[1]` matches an exit code.
- The test title is the nearest heading, so a block's title is its scenario key.
- The chain stops at the first failure only with `defaults.fail_fast: true`, and later blocks are reported as skipped. The proof runner sets it.
- The binary sets `TESTDIR`, `TMPDIR`, `LANG=C`, `LC_ALL=C`, `TZ=GMT`, and `COLUMNS=80` in `src/bin/utils/environment.rs`. The library sets none of them, so the adapter sets the same values, including `COLUMNS=80`, so `scrut test` on a README gives the same output.
- `scrut update` rewrites expected output and keeps headings and prose, so `MarkdownUpdateGenerator` writes transcripts.
- The library exposes `MarkdownParser`, `StatefulExecutor`, `MarkdownTestCaseGenerator`, and `MarkdownUpdateGenerator`. No config struct denies unknown fields.

## tui-test

tui-test 0.1.1 was installed on 24 October 2026 from Microsoft's Homebrew tap (`brew tap microsoft/tui-test https://github.com/microsoft/tui-test`, then `brew install tui-test`, binary sha256 `242fb756103309eae3a8870070e305b444e27737e54149b620a085c318ee9c05`). Runs against bash and against Claude Code with the spec and review plugins showed:

- The daemon cannot start inside the agent sandbox ("Operation not permitted"), so screen steps run on the owner's host, as the specs say.
- `record start x.cast` writes asciicast v2.
- `run` passes the caller's environment to the program. A harness session's environment holds its auth token, base URL, and session markers, so the program starts under `env -i` with `HOME`, `USER`, `PATH`, `TERM`, and `LANG` only. Claude Code then logs in from the keychain, and no inherited session marker turns off its transcripts.
- `--setting-sources project,local` raises the "Allow external CLAUDE.md file imports?" dialog in mods, so the screen part drops it and keeps `--strict-mcp-config`. No mods plugin declares an MCP server.
- A checkout at a path Claude Code has not seen raises the folder trust dialog. The owner trusts the stable checkout path once, and the proof runner never answers a trust or import dialog.
- A dialog's cursor is `❯`, so `❯\s` is no ready marker. Regexes match inside one row, and neither `^` nor `\n` anchors to a row start. The empty prompt's placeholder `❯ Try "` is the startup marker.
- `submit` of a slash command loses its Enter to the command menu. `type`, then `wait idle`, then `key press Enter` runs it.
- A no-break space matches `\s` under `--regex`, or a plain space under `--whitespace normalize`, and never a plain space by default.
