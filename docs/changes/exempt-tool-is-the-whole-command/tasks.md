## 1. Implementation

- [x] 1.1 The shell reader: simple commands, assignments, words with their source text, redirections, substitutions, groups, here-documents, comments
- [x] 1.2 The verdict: the sandbox-off refusal, the refusal with the command to run, the directory option per tool, the advice on joined commands
- [x] 1.3 The exempt list from the arguments, else from the Claude Code settings files while the sandbox is on
- [x] 1.4 The adapter name `sandbox-guard`, and `additionalContext` on Claude `PreToolUse`
- [x] 1.5 CHANGELOG entry, and the handler in the owner's commented `hooks:` block (dotfiles change `qsomtzmu`)
- [x] 1.6 The reader decodes the escapes of a `$'…'` string as Bash does, and counts every substitution in a `${…}` operand whatever quotes or `#` stand in it

## 2. Verification

- [x] 2.1 Unit tests: the reader, each wrapper, each directory option, other tools, the advice cases, the patterns, the settings files, the adapter, the Claude encoding
- [x] 2.2 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
- [x] 2.3 Proof: one scene per scenario through `rune hook run`, 14 of 14 proven
- [ ] 2.4 Live: with the `hooks:` block active, `cd /x && jj log` in a Claude Code session is refused with `jj -R /x log`
