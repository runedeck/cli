## 1. Root choice

- [x] 1.1 `offer_root_choice` in `spec.rs`: single-choice offer on interactive runs, one-line note on non-interactive/`--json` runs, early return when configured, native, or no openspec tree
- [x] 1.2 Persist the answer as `spec.root` in the repo's `config.yaml` (`ontology::set_nested_in_file` made `pub(crate)`)
- [x] 1.3 Hook into `run_spec` for every action except `export`/`import`

## 2. Doctor cross-check

- [x] 2.1 `openspec_cross_check`: run `openspec validate --all --no-interactive` on openspec roots when the binary exists; failures become one advisory warning finding
- [x] 2.2 Verify the warning renders on a failing tree and the exit code stays 0

## 3. Safety hardening

- [x] 3.1 Validator subprocess uses a bounded deadline with kill-on-expiry; a hung `openspec` becomes one advisory warning
- [x] 3.2 Unreadable `config.yaml` skips the offer instead of reading as unset; a symlinked `config.yaml` refuses the write
- [x] 3.3 EOF and unrecognized prompt input record nothing; only an explicit answer persists
- [x] 3.4 Conversion is a true move: symlinked mapping roots refuse, and the source tree is removed once every copy lands

## 4. Verification

- [x] 4.1 Unit tests: configured-root override, non-interactive note writes nothing, configured/native trees skip the offer, malformed config untouched, symlinked roots refused, move removes the source tree
- [ ] 4.2 `cargo fmt`, `cargo clippy --all-targets`, full test suite green
- [x] 4.3 Manual Testing spec section documents the fallback and cross-check
