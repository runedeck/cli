# Tasks

## 1. Record

- [x] 1.1 `src/cli/hook/record.rs`: the record line type (`v` 1), the monthly path under `state::dir().join("hooks/record")`, directory creation, append with `O_APPEND` and no fsync, the bounded writer thread, a failure or a late write as one stderr line naming the records lost
- [x] 1.2 `src/cli/hook/run.rs`: collect one record per hook it runs or fails to start, fill in the dispatcher answer once the event is answered, and write them after the answer is encoded within the reserve kept for the answer and record
- [x] 1.3 Tests: two hooks in order, the budget case, the month boundary, the empty state dir, the read-only directory, the blocked writer, each with the answer and the exit code unchanged

## 2. Metrics

- [x] 2.1 `src/cli/hook/encode.rs`: `HandlerResult.metrics: Option<Map>`, the identifier rule `[A-Za-z0-9._:/-]{1,64}`, integer values, the 4096-byte cap on the UTF-8 serialization, encoders ignore the field
- [x] 2.2 `src/cli/hook/adapters.rs`: `lint_on_write_in` counts `rules` and `cause` from the same lines it reports, through one parser shared with the import
- [x] 2.3 Tests: metrics copied, text in a value and in a key dropped, 4096 kept and 4097 dropped, the em-dash-semicolon-typo fixture, the clean fixture, no message text and no misspelled word in the record

## 3. Read commands

- [x] 3.1 `src/cli/hook/stats.rs`: load and filter (`--since`, `--until` as inclusive UTC periods, `--harness`, `--hook`, `--rule`, `--file`, `--asked`, `--answer`, all combined), the list output, the aggregates (runs by what the hook asked for and by answer, rules with cause and sole cause and advice, weeks, repeats against the immediately preceding denial, files, hooks), `--top` by count then name, `--json` with the same figures
- [x] 3.2 `src/cli/hook/import_claude.rs`: walk `**/*.jsonl`, the attachment filter, the shared finding parser, the checkpoint `import-claude.json` with per-file offset after the last complete line and the kept tool-use ids, dedupe on harness, session, tool-use id, and hook against live and imported records
- [x] 3.3 `src/cli/mod.rs`: `HookAction::Record { stats | import }`, `tests/fixtures/root-help.txt`
- [x] 3.4 Tests: the fixture table, among-the-causes versus sole cause, the top tie, JSON equals table, repeat denials, the two-transcript import run twice, the live record already holding the tool use, the grown transcript, the earlier tool use, the half-written last line, the empty record
- [x] 3.5 Check that `rune release`, `rune bench report`, `rune proof run`, and `rune sign` read nothing under `hooks/record/` and carry no record line in what they write, with one test per command

## 4. Verification

- [x] 4.1 Astra on the proposal, the two delta specs, and `adr.md` before code, dispositions in `design.md`
- [x] 4.2 `rune proof scaffold dispatcher-records-every-hook-execution`, 26 of 27 scenarios recorded with `rune proof run` against fixtures. `disk-blocks-the-writer` stays unproven in the cast and is covered by a unit test with an injected stalled writer. The live record is no proof input
- [ ] 4.3 Astra and Grok on the code after group 3, dispositions in `design.md`
- [ ] 4.4 `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo test --all-features`, `prek run --all-files` and `--stage pre-push` in an isolated clone, `rune spec validate dispatcher-records-every-hook-execution`, receipt, `rune sign queue`

## 5. Record and retirement

- [x] 5.0 Terms, deck side: hook, hook dispatcher, hook execution, hook record, hook metrics, and dispatcher answer defined in `ontology/rune.ttl` after the owner's review, on the deck's main at `861ae76c` (2026-10-17)
- [x] 5.0a Terms, cli side: the deployed ontology from the cli change `install-deck-ontology` (`.rune` key `ontology: deck`, `rune install` writes `ontology/rune.ttl` with its sidecar), on main at `f721ba57425f`. This change adds nothing for it
- [x] 5.0b Citations: each first use in the two specs, the proposal, and `adr.md` set in italics with its `[TERM]` link, link definitions at the end of each file, `rune spec validate dispatcher-records-every-hook-execution` passes with the term lint active
- [x] 5.1 `scripts/sync-idea` copied from the deck, `docs/ideas/Hooks Record Log.svg` and its sidecar written, `--check` passes
- [x] 5.2 Retention: monthly files are kept, no pruning in this release (owner, 2026-10-17). A month at the audited rate stays under 4 MB even when every record reaches the metrics cap
- [ ] 5.3 After the merge: `rune hook record import --harness claude`, then drop dotfiles `change/lint-stats` and update the setup workshop memory note `lint-stats-store`
- [ ] 5.4 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number, CHANGELOG entries
