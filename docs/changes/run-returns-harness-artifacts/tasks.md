## 1. Implementation

- [x] 1.1 `SurfaceReply.conversation_id` from agy's JSON, `None` elsewhere
- [x] 1.2 `src/cli/run/artifacts.rs`: the data directory, the top-level filter with the bookkeeping and the start-time rule, the copy with a numeric suffix, the state folder
- [x] 1.3 `rune run --artifacts-dir <DIR>`, the clean-run default under `$XDG_STATE_HOME/rune/artifacts/<conversation_id>/`, the `artifacts` key, the `artifact:` lines, the dry-run `artifacts_dir:` line
- [x] 1.4 README and CHANGELOG

## 2. Verification

- [x] 2.1 Unit tests: bookkeeping and old files skipped, missing folder and empty id give an empty list, a clash gets `-1`, `-2`
- [x] 2.2 Integration tests with a fake agy: the image reported, bookkeeping skipped, `--artifacts-dir` copy with a clash, a clean run keeping the file, grok reporting an empty list, the dry run naming the destination
- [x] 2.3 `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --bin rune`, `cargo test --test run`
- [ ] 2.4 `rune spec validate run-returns-harness-artifacts`
- [ ] 2.5 Live: `rune run sol@agy --clean "Draw a 64 by 64 icon"` returns the image under `~/.local/state/rune/artifacts/<conversation_id>/`
