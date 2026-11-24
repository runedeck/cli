# Tasks

## 1. Cursor as a harness

- [x] 1.1 `src/hooks.rs`: `Harness::Cursor`, seven native events, `notification` unsupported, `can_block` and the capabilities Cursor accepts
- [x] 1.2 `src/cli/hook/payload.rs`: decode Cursor's payload
- [x] 1.3 `src/cli/hook/encode.rs`: `encode_cursor`, a JSON object on every path
- [x] 1.4 Tests: `cursor_maps_seven_events_and_lacks_notification`, `a_cursor_payload_reads_its_conversation_tool_and_loop_count`, `cursor_answers_in_its_own_shape_and_never_in_silence`

## 2. Install beside foreign entries

- [x] 2.1 `src/cli/hook/install.rs`: the flat table wrapped and unwrapped, `failClosed` on a guarded event, no `type`, no adoption on Cursor, `version` kept or added on write
- [x] 2.2 `src/cli/hook/table.rs`: `ensure_member`
- [x] 2.3 Tests: `cursor_entries_sit_beside_the_guard_and_git_ai_and_a_second_install_changes_nothing`, `a_fresh_cursor_table_gets_its_version_and_a_dry_run_writes_no_table`, `a_missing_member_is_added_once_and_a_present_one_keeps_its_text`

## 3. Harness choice

- [x] 3.1 `src/cli/hook/install.rs`: `prepare_in` writes the selected harnesses and keeps the others' tables, records, and named generations
- [x] 3.2 `src/cli/hook/mod.rs`, `src/cli/mod.rs`: `--harness` on the install, `rune hook remove`, `Mode`
- [x] 3.3 Test: `an_install_into_one_harness_leaves_the_others_their_tables_and_generations`
- [x] 3.4 Review 2026-11-02: an unselected harness's table is not read, locked, or created (`HarnessManifest.generations`, `recorded_generations`, selected-only `table_files`, `named_generations`, `sweep_generations`); the full install attempts the three registered harnesses and skips one whose folder is missing (`present`, `Report.skipped`, `HookSummary.skipped_harnesses`); `rune install` follows the same rule
- [x] 3.5 Tests: `an_unselected_harness_s_generations_come_from_the_manifest_not_its_table`, `an_unselected_harness_s_table_is_neither_read_nor_locked_nor_created`, `a_harness_without_its_folder_is_skipped_and_returns_with_it`

## 4. Per-harness switch

- [x] 4.1 `src/hooks.rs`: `Switches`, `SwitchedOff`, `Plan.off`, `compile_with`
- [x] 4.2 `src/cli/hook/switches.rs`: the state file, harness name parsing, the verb's work
- [x] 4.3 `src/cli/hook/mod.rs`, `src/cli/mod.rs`: `compile_switched`, the `off:` line in the list, `--harness` on `on` and `off`, `hook_switch`
- [x] 4.4 Tests: `a_switch_leaves_a_handler_out_of_one_harness_only`, `the_plan_names_each_switched_off_handler_once`, `a_handler_switched_off_in_one_harness_leaves_that_harness_s_entry_unguarded`, the two in `switches.rs`
- [x] 4.5 Review 2026-11-02: `switches::route` honors `--provider` and `--harness` before the name and refuses a bare name both a handler and a staged rune carry (`hooks.ambiguous_name`); `switch` runs the install for the registered harnesses it named
- [x] 4.6 Tests: `a_flag_routes_before_the_name_and_a_bare_double_name_is_refused`, `an_explicit_provider_never_switches_a_handler`, `a_switch_rewrites_the_harness_table_at_once`

## 6. Review findings 2026-11-02, the dispatcher on Cursor

- [x] 6.1 `src/cli/hook/adapters.rs`: `is_shell_tool` reads Cursor's `Shell` for dcg and rtk; test `dcg_reads_cursor_s_shell_tool_as_bash`, `dcg_judges_cursor_s_shell_tool_through_the_dispatcher`
- [x] 6.2 `src/hooks.rs`, `src/cli/hook/adapters.rs`: `Capability::supported_on` per event; the postbox leaves a message posted on a Cursor `beforeSubmitPrompt`; tests `context_is_a_capability_per_event`, `hook_keeps_a_cursor_prompt_message_posted_and_delivers_it_after_a_tool`
- [x] 6.3 `src/cli/hook/run.rs`: `dispatcher_failure` and `unregistered` answer through the encoder; test `a_cursor_registration_answers_allow_where_it_has_nothing_to_run`
- [x] 6.4 `src/cli/hook/encode.rs`: a passive permission extension answers `{"permission": "allow"}`; tests `a_cursor_permission_extension_answers_allow_and_an_observer_an_object`, `a_cursor_permission_extension_observed_passively_answers_allow`

## 7. Review findings 2026-11-02, round two

- [x] 7.1 `src/cli/hook/run.rs`, `src/cli/hook/switches.rs`: a firing applies the switches beside its plan generation, so a Codex entry kept trusted stops running a switched-off handler; test `a_codex_switch_holds_where_codex_keeps_the_trusted_entry`; the choice in the design
- [x] 7.2 `src/cli/hook/switches.rs`, `src/cli/hook/publish.rs`: one lock across the read, the change, the install, and the write, the lock re-entrant per thread, the state put back when the install fails; tests `a_switch_waits_for_the_lock_before_it_reads_the_state`, `a_switch_the_install_refuses_leaves_the_state_as_it_was`, `a_thread_that_holds_a_directory_locks_it_again_without_waiting`
- [x] 7.3 `src/hooks.rs`, `src/cli/hook/encode.rs`, `src/cli/hook/adapters.rs`: context and the postbox reach `postToolUseFailure`; tests `context_reaches_cursor_s_tool_failure_event`, `hook_delivers_to_a_cursor_tool_failure_and_keeps_a_message_on_another_extension`, `context_is_a_capability_per_event`
- [x] 7.4 `src/cli/hook/payload.rs`: `tool_input` parsed when it is a JSON string; test `a_cursor_tool_input_sent_as_a_json_string_is_parsed`
- [x] 7.5 `src/cli/hook/mod.rs`, `src/cli/install/mod.rs`: the skip report on the nothing-to-do path (`Report.nothing`); test `a_fresh_config_still_reports_the_skipped_harnesses`

## 8. Review findings 2026-11-02, round four

- [x] 8.1 `src/cli/hook/mod.rs` (`InstallTarget`), `src/cli/hook/switches.rs`: every writer takes the install's whole lock set in one order; the switch takes it up front; test `a_switch_takes_the_install_s_whole_lock_set_in_the_install_s_order`
- [x] 8.2 `src/cli/hook/mod.rs`: the switches are read and the plan compiled under the install lock; test `an_install_reads_the_switches_under_its_lock`
- [x] 8.3 `src/cli/hook/run.rs`, `src/cli/hook/switches.rs`: a switches file that is there and does not read follows the registration's policy; the read is bounded to a regular file under 64 KiB; tests `a_switches_file_that_does_not_read_follows_the_registration_s_policy`, `a_switches_path_that_is_no_small_regular_file_is_refused`
- [x] 8.4 `src/hooks.rs`, `src/cli/hook/switches.rs`: `Switches` read strictly (`deny_unknown_fields`, both members, version 1); a refused switch restores the file's exact bytes and mode, or its absence (`Original`); tests `a_switches_file_is_read_strictly`, `a_refused_switch_puts_the_switches_file_back_as_it_was`
- [x] 8.5 `src/cli/hook/switches.rs`: `path_beside_plan` anchored to the canonical plan path, a relative plan refused; tests `the_switches_sit_beside_the_plan_and_a_relative_plan_is_refused`, `a_firing_reads_the_switches_beside_its_plan_whatever_the_state_directory_says`
- [x] 8.6 `src/cli/hook/publish.rs`: `Lock` is not `Send`; test `a_lock_stays_on_the_thread_that_took_it`

## 9. Review findings 2026-11-02, round five

- [x] 9.1 `src/cli/hook/mod.rs`: `rune hook list --write` takes the install's whole lock set before it compiles and holds it through the publish; test `a_listing_that_writes_compiles_under_the_install_s_lock`
- [x] 9.2 `src/cli/hook/publish.rs`, `tests/hook_dispatch.rs`: `RUNE_HOOK_LOCK_TRACE` writes `wait`/`held` lines; the three concurrency tests wait for the child's `wait` line instead of sleeping (`waits_for`)
- [x] 9.3 `src/cli/hook/switches.rs`: `path_beside_plan` reads the absolute plan path lexically, so a `plans` link keeps its name; tests `the_switches_sit_beside_a_plans_link_by_its_name`, `a_firing_finds_the_switches_beside_a_plans_link_by_its_name`
- [x] 9.4 `src/cli/hook/switches.rs`: `read_at` uses `symlink_metadata` and refuses a link at the path; tests `a_symbolic_link_at_the_switches_path_is_refused`, the dangling-link case in `a_switches_file_that_does_not_read_follows_the_registration_s_policy`
- [x] 9.5 `src/cli/hook/encode.rs`, `run.rs`, `record/mod.rs`: the firing record's optional `unapplied` member names the effects the answer could not carry, filled where the "reported, not applied" line is printed, stored on the hooks that asked, absent when nothing was dropped; test `a_firing_records_the_context_cursor_s_stop_has_no_field_for`
- [x] 9.6 `src/cli/hook/run.rs`: `merge` returns the passive event's denial and replacement input it discards, and `settle_unapplied` stores them with the encoder's drops; test `a_firing_records_what_a_passive_event_s_merge_discarded`. `src/cli/hook/encode.rs`: a replacement input on any event but `tool.before` is reported and dropped like the rest; test `a_replacement_input_off_tool_before_is_reported_and_stored_as_unapplied`
- [x] 9.7 `tests/hook_dispatch.rs`: `a_switch_waits_for_the_lock_before_it_reads_the_state` waits on the lock trace (`waits_for`) instead of sleeping

## 5. Verification

- [x] 5.1 `cargo fmt --all`, `cargo clippy --all-targets` clean, `cargo test --bin rune hook`, `cargo test` whole
- [x] 5.2 `tests/hook_dispatch.rs`: `named_generations` reads the flat Cursor table
- [ ] 5.3 The owner runs `rune hook install --check` against the live `~/.cursor/hooks.json` and reads the diff before the first write
