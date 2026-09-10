# Acceptance contract

Implementation status and exact validation results live in [tasks](tasks.md) and [validation](validation.md).
The check names below are the contract for implementation and review.
Keep their fixtures outside the worker's editable acceptance inputs after the coordinator freezes the task.

## Structured findings

Use these codes in structured inspection, migration errors, and direct metadata-retention assertions.
`CSI004_METADATA_LOSS` is the metadata regression assertion. `CSI005_MIGRATION_CONFLICT` is a migration refusal.
Do not create broad text bans for `.codex` or provider names.
The separate finite literal-symbol policy includes quoted examples without exemptions.

| Finding code | Fail condition | Requirement |
| --- | --- | --- |
| `SL000_INVALID_INPUT` | The standalone source gate cannot establish its requested source scope or configured model registry | CSI-08 |
| `CSI001_DUPLICATE_NAME` | Two distinct applicable entries declare the same selected skill name, including equal content | CSI-02 |
| `CSI002_INCOMPLETE_IDENTITY` | Selected source, destination, ownership classification, or required discovery scope cannot be established | CSI-01, CSI-02 |
| `CSI003_INVALID_BUNDLE` | Missing or changed selected content, broken required local reference, invalid path, or escaping/cyclic symlink | CSI-03 |
| `CSI004_METADATA_LOSS` | A retained resolved metadata value disappears or changes after variant merging | CSI-04 |
| `CSI005_MIGRATION_CONFLICT` | Legacy ownership is unproven, local/unowned content would move, a foreign destination is occupied, or an obsolete copy remains | CSI-05 |
| `CSI006_INVALID_EVIDENCE` | Required result is missing, skipped, malformed, stale, or inconsistent with the inspected candidate | CSI-06, CSI-07 |
| `CSI007_HARNESS_LEAKAGE` | Source or rendered text contains an enumerated forbidden harness symbol or runtime frontmatter key | CSI-08 |
| `CSI008_LAYER_SOURCE` | Required source is malformed, unreadable, empty, or unsafe to inspect | CSI-08 |
| `CSI009_LAYER_PATH` | A source qualifier is unknown, misplaced, or uses a model outside its configured provider | CSI-08 |
| `CSI010_LAYER_METADATA` | A variant omits explicit mode or violates its layer's identity, routing, or runtime metadata rules | CSI-08 |

Readiness findings include the selected identity and affected paths where available.
Source findings identify their authored layer and path. Standalone input failures identify the requested source context.
Incomplete scope is an explicit failing condition for readiness, even if ordinary doctor inspection can continue.
Ordinary installed/stale/missing listing semantics remain unchanged.
The acceptance checker must inspect findings. General doctor exit zero is insufficient.
The native acceptance record uses `CSI006_INVALID_EVIDENCE` when its own evidence is incomplete.
Doctor does not invent a native verdict or start a model session.

## Deterministic regression tests

Use `tests/codex_skill_identity.rs` in the CLI implementation workspace.
Reuse existing tests in `tests/deploy.rs`, provider tests, transform tests, and deploy ownership tests where applicable.
Keep these 18 named cases identifiable even if existing helpers supply most of their setup.

| ID and contract test name | Required positive and negative evidence | Requirement |
| --- | --- | --- |
| T01 `codex_default_skills_use_by_kind_target` | Skill goes to `.agents/skills/AlphaSkill`; native agent keeps its private destination | CSI-01 |
| T02 `explicit_skill_target_is_preserved` | Configured override remains effective; incorrect default assumption fails target acceptance | CSI-01 |
| T03 `equal_duplicate_names_are_ambiguous` | One entry passes; separate user/project entries with equal content fail | CSI-02 |
| T04 `declared_name_controls_duplicate_detection` | Different names pass; divergent copies and renamed folders with one declared name fail | CSI-02 |
| T05 `incomplete_scope_cannot_pass` | Complete roots pass; inaccessible roots or unresolved selection fail with a scope finding | CSI-01, CSI-02 |
| T06 `bundle_digest_covers_companions_and_entry_types` | Unchanged bundle is stable; companion, binary byte, executable bit, or symlink-target mutation changes its digest | CSI-03 |
| T07 `complete_selected_bundle_deploys` | Nested links, script sibling import, binary asset, and authored `agents/openai.yaml` survive; excluded skill leaves no companions | CSI-03 |
| T08 `portable_metadata_survives_codex_assembly` | Resolved parsed values survive; dropping one fails; valid variant overrides and absent optional fields pass; directives stay filtered | CSI-04 |
| T09 `only_proven_obsolete_copy_is_quarantined` | Valid ownership permits migration; matching name/hash without ownership cannot permit it | CSI-05 |
| T10 `migration_preserves_foreign_content` | Empty destination permits migration; foreign destination, unknown sidecar, or untracked addition stays byte-identical | CSI-05 |
| T11 `migration_preserves_local_edits` | Unchanged managed bundle migrates; edited entrypoint or companion remains recoverable and unresolved | CSI-05 |
| T12 `retained_legacy_copy_blocks_unique_readiness` | Completed migration clears its finding; `--no-prune` retains content and its finding | CSI-02, CSI-05 |
| T13 `second_install_is_identity_stable` | Existing valid managed destination permits an unchanged second install with no content, manifest, provenance, or quarantine changes | CSI-07 |
| T14 `failed_migration_preserves_recovery` | Success updates claims; injected failure preserves recoverable content; retry handles a valid managed destination without a false foreign conflict | CSI-05, CSI-07 |
| T15 `shared_skill_target_has_no_competing_writer` | All enabled relevant providers preserve each other's files across two installs; conflicting targets cannot pass acceptance | CSI-01, CSI-07 |
| T16 `modified_doctor_finding_overrides_zero_exit` | Clean structured findings pass; modified selected file fails despite general doctor exit zero | CSI-03, CSI-07 |
| T17 `missing_skipped_or_stale_results_fail` | Complete current result passes; missing executable, zero discovered tests, required skip, malformed JSON, or stale digest fails | CSI-06, CSI-07 |
| T18 `case_aliases_and_symlinks_are_accounted_for` | Valid internal reference passes; case collision, escape, cycle, or duplicate native entry through an alias fails | CSI-02, CSI-03 |

## Implemented test map

This table maps the contract to current test assertions. Execution results remain separate in
[validation](validation.md).

The prefixes identify these files:

- `I`: [identity integration](../../../tests/codex_skill_identity.rs).
- `M`: [metadata and companion integration](../../../tests/codex_skill_metadata.rs).
- `D`: [deployment unit controls](../../../src/cli/deploy/tests.rs).
- `B`: [bundle unit controls](../../../src/manifest/bundle/tests.rs).
- `A`: [assembly unit controls](../../../src/assemble/tests.rs).
- `N`: [raw native protocol controls](../../../src/skill_readiness/native/tests.rs).
- `P`: [native adapter controls](../../../scripts/test_check_codex_skill_discovery.py).

| ID | Implemented tests | Remaining contract evidence |
| --- | --- | --- |
| T01 | `I::codex_default_skills_use_by_kind_target`, `D::codex_defaults_use_shared_skills_and_keep_identity_fields`; existing `install_generates_valid_codex_toml_with_effort` in `tests/deploy.rs` | Final candidate execution |
| T02 | `I::explicit_skill_target_is_preserved`; existing `install_routes_content_kinds_to_target_map_roots` in `tests/deploy.rs` | Final candidate execution |
| T03 | `I::equal_duplicate_names_are_ambiguous`, `P::test_catalog_preserves_equal_name_duplicates`, `P::test_catalog_is_sorted_without_selecting_a_winner` | Final candidate execution |
| T04 | `I::declared_name_controls_duplicate_detection`, `I::declared_names_control_divergent_and_distinct_candidates`; both duplicate fixtures check reversed-root ordering | Finish exact affected-path assertions |
| T05 | `I::incomplete_scope_cannot_pass`, `I::missing_whole_managed_bundle_cannot_hide_behind_a_valid_skill`, `I::current_source_changes_invalidate_deployed_selection` | Complete exact code/path assertions for each unavailable-scope control |
| T06 | `I::bundle_digest_covers_companions_and_entry_types`; `B::companion_bytes_paths_and_binary_changes_invalidate_identity`, `B::executable_bits_are_part_of_the_bundle_identity`, `B::contained_symlink_target_and_entry_type_change_identity`, `B::provenance_content_and_timestamps_do_not_change_identity` | Final candidate execution |
| T07 | `I::complete_selected_bundle_deploys`, `M::codex_selected_companions_remain_runnable_and_excluded_bundles_stay_absent` | Final candidate execution |
| T08 | `I::portable_metadata_survives_codex_assembly`, `M::codex_assembly_preserves_absent_optional_metadata`, `M::codex_variant_replaces_nested_metadata_without_superseded_keys` | Final candidate execution |
| T09 | `D::codex_migration_moves_complete_bundle_and_repeats_without_changes`, `D::codex_migration_preserves_conflicts_before_any_content_write_even_with_force` | Complete affected-path assertions for ownership refusals |
| T10 | `D::codex_migration_preserves_conflicts_before_any_content_write_even_with_force`, `D::codex_migration_preserves_yaml_that_looks_like_provenance` | Complete affected-path assertions for foreign-content refusals |
| T11 | `D::codex_migration_preserves_conflicts_before_any_content_write_even_with_force`, `D::modified_file_keeps_install_semantics_but_invalidates_source_snapshot` | The edit fixture covers a companion; add the corresponding entrypoint and affected-path controls |
| T12 | `D::codex_migration_no_prune_keeps_legacy_claims_and_then_resumes`, `I::no_prune_install_remains_unverified_until_complete_install` | Directly assert duplicate readiness before and after the legacy-copy transition |
| T13 | `I::second_install_is_identity_stable`, `D::codex_migration_moves_complete_bundle_and_repeats_without_changes`, `D::source_snapshot_deploys_only_after_complete_install_and_stays_outside_manifest` | Final serialized execution with stable source and executable bytes |
| T14 | `D::codex_migration_public_failure_restores_destination_and_retries`, `D::codex_migration_recovers_failed_copy_and_retries_without_foreign_loss`, `D::codex_migration_restores_legacy_bundle_when_manifest_update_fails`, `D::codex_migration_keeps_legacy_when_replacement_changes_after_preflight` | Focused controls passed; final serialized suite pending |
| T15 | `D::codex_and_agentskills_distinct_roots_coexist_across_two_installs`, `D::codex_shared_root_refuses_multiple_selected_writers_before_target_creation`, `D::codex_shared_root_refuses_a_different_recorded_writer` | Both distinct-root configurations passed; final serialized suite pending |
| T16 | `I::modified_doctor_finding_overrides_zero_exit` | Final candidate execution |
| T17 | `I::missing_skipped_or_stale_results_fail`, `N::empty_or_malformed_records_cannot_pass_native_validation`, `P::test_missing_executable_records_failure_without_skipped_success` | Trusted external-runner and zero-discovered-test controls remain unproven |
| T18 | `I::case_aliases_and_symlinks_are_accounted_for`; `B::case_collision_detection_does_not_depend_on_host_filesystem`, `B::contained_directory_aliases_preserve_local_references`, `B::escaping_dangling_and_cyclic_symlinks_are_rejected` | Final Linux/macOS jobs and native duplicate-through-alias observation |

The [skill-readiness workflow](../../../.github/workflows/skill-readiness.yaml) adds Linux and macOS jobs for both
integration targets and the Python adapter controls.
Its definition does not establish a completed platform run.
The finding codes and required assertions above remain acceptance requirements.
The table does not convert missing controls into passing checks.

T14 exercises public deployment failure, rollback, removal of the failure injection, successful public retry, and an
unchanged repeat. Its copy-failure matrix covers absent and previously managed replacements, unrelated claims, and
preserved foreign additions in the reported recovery directory.
T15 checks default Codex with a custom AgentSkills root and custom Codex with the default AgentSkills root.
Both configurations preserve their content across two installs.

## Source-layer and rendered controls

These controls extend CSI-08 without replacing T01–T18 or the native cases.
The source command is `rune validate --skill-layers --source <skill-folder|module|deck>`.
With `--json`, it returns a `rune-skill-source-layers/v1` report with `root`, `checked`, `findings`, and `valid`.
`checked` counts inspected text paths, including source aliases, rather than model calls or unique skills.
Each finding identifies the layer, source path, relevant line or key when available, and stable finding code.
Zero checked skills, unsupported input, or an unavailable required check cannot pass.
The named tests below map these controls. Their execution results remain separate in the validation record.

| ID | Required positive and negative evidence |
| --- | --- |
| L01 Generic source | Portable Agent Skills fields and existing source routing/invocation controls pass. Other enumerated runtime frontmatter fails in generic entrypoints. Harness-only symbols fail in entrypoints and shared companions. |
| L02 Harness source | A supported harness permits its own symbols and declared metadata. Foreign symbols and runtime keys fail, with the exact path and token or key. |
| L03 Model source | An exact configured provider/model path with mode-only frontmatter passes. Unknown IDs, misplaced models, and identity, routing, or runtime metadata changes fail. |
| L04 Explicit modes | Harness/model append, prepend, and replace pass when resolved content satisfies policy. Metadata-only append/prepend may have empty bodies. Missing or invalid modes fail. User replacements require explicit replace and complete canonical identity. |
| L05 Single-winner resolution | Only the winning harness/model variant combines with the base. A model body does not accumulate the provider body. User entrypoints replace the complete base before variant resolution, preserving canonical identity and using their own targets. |
| L06 Companions and literals | Shared text companions and user overrides receive the portable policy and resolved harness checks. Quoted examples and conditional text fail for exact forbidden symbols. Provider names and `.codex` mentions alone pass. |
| L07 Rendered leakage | Clean rendered Codex content passes. Leaks in the resolved entrypoint, retained append/prepend base text, and emitted companions produce `CSI007_HARNESS_LEAKAGE`. Filtering a source key cannot hide invalid source placement. |
| L08 Complete inspection | Skill, module, and deck roots inspect skills and report text-path counts. Empty, malformed, unreadable, unsafe, or unsupported sources fail. Invalid nonwinning variants and empty or unknown target lists fail. |
| L09 Read-only operation | Checks preserve source, build, and deployed bytes and do not fetch sources or start a model. Ordinary validation behavior remains unchanged. |
| L10 Boundary controls | The symbol policy handles explicit token boundaries and does not turn substrings, provider names, or paths into broad bans. Instruction encoding errors fail. Binary assets remain covered by bundle identity. |
| L11 Configuration matching | Configured aliases and target directories pass. One content selector checks every matching provider. Unknown selectors and malformed provider configuration fail without fallback. CLI provider arguments independently require a unique provider identity. |
| L12 Missing input | An unavailable source returns the versioned report with `SL000_INVALID_INPUT`, its requested path, zero checked paths, and `valid: false`. |

Use the same reviewed literal policy in source checks and rendered doctor checks.
Inspect examples literally. Do not add comments, prose conditions, or file-name exemptions that suppress policy matches.
Assert the relevant code, path, and token or key for each invalid control.
The policy verifies enumerated symbols and declared metadata. It does not infer arbitrary semantic harness dependencies.
Model checks do not prove tool availability, model quality, or native execution.
Do not simulate provider/model companion merging, which remains outside the existing assembly contract.

### Literal policy boundary

The implementation uses these finite sets. It does not claim a complete catalog of every harness capability.
The bounded frontmatter restrictions are not a universal allowlist for every possible source key.
Symbol matches ignore ASCII case and require identifier boundaries.
Syntax matches preserve case. Named call syntax also requires a preceding identifier boundary.

| Harness | Enumerated symbols |
| --- | --- |
| Claude | `AskUserQuestion`, `TodoWrite`, `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`, `TaskOutput`, `TaskStop`, `EnterPlanMode`, `ExitPlanMode`, `NotebookEdit`, `subagent_type`, `run_in_background`, `CLAUDE_SKILL_DIR`, `CLAUDE_SESSION_ID` |
| Codex | `request_user_input`, `exec_command`, `write_stdin` |
| Gemini | `run_shell_command`, `ask_user` |

Claude syntax includes an exclamation mark immediately followed by a backtick.
It also includes `Bash(`, `Read(`, `Write(`, `Edit(`, `Glob(`, `Grep(`, `Agent(`, and `Skill(`.
Generic and user layers reject all enumerated harness symbols and syntax.
Harness and model layers permit their own set and reject the other sets.
AgentSkills and OpenCode have no additional harness-only symbols in this initial policy.
Do not interpret that absence as complete static coverage for those harnesses.

The rendered Codex policy rejects these top-level entrypoint keys:
`context`, `agent`, `model`, `hooks`, `argument-hint`, `disable-model-invocation`, `user-invocable`, `effort`,
`disallowed-tools`, `background`, `shell`, `paths`, `arguments`, and `when_to_use`.
Source rules retain existing `targets`, `disable-model-invocation`, and `user-invocable` assembly controls.
Target lists must be nonempty and match configured provider names, aliases, or target directories.
They use `ProviderConfig::matches_target` and preserve every matching provider.
CLI provider arguments use exact key, then explicit alias, then unique target-directory precedence.
They reject ambiguity with sorted candidate names before work starts.
The unique CLI resolver must not narrow per-rune content selectors.
Model frontmatter permits only `mode`.
User entrypoints require `mode: replace`, a nonempty body, complete identity, and the unchanged canonical skill name.
They do not inherit omitted fields from the canonical entrypoint.
The portable `allowed-tools` key is permitted, but its text values still receive the literal-symbol and syntax checks.
These checks do not establish permission enforcement.

### Layer test map

The following tests are authored controls. See [validation](validation.md#layer-gate-results) for observed execution results.
`S` denotes [source-layer integration](../../../tests/skill_layers.rs).
`R` denotes [rendered portability integration](../../../tests/codex_skill_portability.rs).
`U` denotes [literal policy unit tests](../../../src/skill_readiness/portability/tests.rs).
`G` denotes [source-layer unit tests](../../../src/skill_readiness/layers/tests.rs).
`C` denotes [source-configuration integration](../../../tests/skill_layer_config.rs).
`V` denotes [provider selection unit tests](../../../src/provider/tests.rs).

| Cases | Authored tests |
| --- | --- |
| L01, L02, L09 | `S::generic_and_claude_layers_pass_without_changing_source`, `S::codex_layer_rejects_claude_tool_and_parameter`, `U::ordinary_words_and_portable_metadata_remain_valid`, `G::source_routing_directives_survive_until_actual_rendering` |
| L03 | `S::exact_model_variant_inherits_harness_checks`, `S::model_variant_cannot_change_skill_identity`, `S::unknown_and_misplaced_model_folders_fail`, `G::model_layer_cannot_override_identity_routing_or_runtime_metadata` |
| L04, L05 | `S::variant_requires_explicit_mode`, `S::model_layer_does_not_inherit_the_harness_variant_body`, `G::effective_body_uses_actual_append_prepend_replace_semantics`, `G::user_then_model_then_harness_selects_one_body_winner`, `G::metadata_only_harness_append_and_prepend_keep_base_body`, `G::user_entrypoint_requires_complete_replacement_and_preserves_name`; existing `A::resolve_provider_model_takes_precedence_over_provider` |
| L06 | `S::generic_companion_cannot_hide_harness_calls`, `U::examples_quotes_and_conditionals_do_not_suppress_literals`, `G::user_companion_replaces_same_relative_base_path` |
| L07 | `R::resolved_codex_variant_excludes_the_claude_procedure`, `R::installed_companion_leak_blocks_strict_readiness`, `U::rendered_companions_are_scanned_but_provenance_and_binary_assets_are_not`, `U::rendered_codex_companions_reject_gemini_symbols` |
| L08 | `S::malformed_model_registry_fails_without_embedded_fallback`, `S::empty_provider_model_list_fails`, `S::deck_selection_uses_the_deck_model_registry`, `S::empty_selection_fails_instead_of_reporting_zero_checks_as_success`, `S::symlinked_skills_container_is_not_followed`, `U::malformed_entrypoint_metadata_fails_visibly`, `U::unreadable_or_changed_relevant_files_fail_visibly` |
| L10 | `U::symbols_match_case_insensitively_at_identifier_boundaries`, `U::claude_command_injection_and_exact_tool_calls_are_literal_violations`, `U::escaping_or_replaced_symlinks_never_expose_external_text`, `U::a_replaced_parent_cannot_redirect_scanning_into_provenance` |
| L11 | `G::configured_content_target_directories_and_aliases_are_valid`, `G::content_target_checks_keep_every_matching_provider`, `G::empty_provider_configuration_cannot_skip_effective_checks`, `C::source_gate_uses_configured_aliases_and_target_directories`, `C::source_target_directory_can_select_multiple_providers`, `C::invalid_provider_configuration_cannot_fall_back_to_defaults`, `C::deck_scope_uses_the_deck_provider_configuration` |
| L11 CLI distinction | `V::provider_alias_wins_over_a_shared_target_path`, `V::shared_target_selector_is_rejected_without_a_unique_identity`, `V::exact_provider_name_wins_over_another_providers_alias`, `V::duplicate_aliases_fail_with_sorted_candidates`, `V::provider_selectors_deduplicate_names_and_keep_unknown_diagnostics` |
| L12 | `C::missing_source_returns_the_versioned_input_report` |

The focused tests and implementation review cover the layer matrix's declared static rules.
The map alone does not authenticate an external runner or establish native model behavior.
The CLI invalid cases now assert relevant codes, tokens, and paths.
The model-layer integration case also asserts the actual assembled body without cumulative provider text.
Earlier focused integration targets passed before the consistency corrections.
The new configuration/input controls pass their focused integration run.
The final immutable publication check remains pending in this record.

Run the source gate against the existing deck and report its baseline findings separately.
The expanded content PR resolves SafetyFirst, the five measured baseline skills, and affected authoring guidance.
It must satisfy its own source gate and fresh baseline without absorbing unrelated Claude-owned work.
Baseline violations remain failures. They are neither automatic exceptions nor evidence that the checker is broken.

## Additional fixture requirements

T18 requires a case-insensitive filesystem job and an explicit filesystem probe.
A missing platform case is incomplete acceptance, not a silent skip.
Run the platform-neutral cases on Linux and macOS.
Reverse enumeration order in T03 and T04 and assert identical sorted findings.
Change only provenance timestamps in T06 and assert unchanged content identity while keeping provenance validation
separate.

Extend T01 and T07 with a shared companion and provider-qualified entrypoint fixture.
Assert one resolved `SKILL.md`, the shared companion, and no emitted skill qualifier directory.
Reuse existing append, prepend, replace, precedence, and frontmatter merge tests.
The replacement case must omit base-only body text. The append case must preserve base text and include the additive
text once.
Verify that only the winning harness/model qualifier combines with the base when both qualifiers exist.
Verify complete user replacement separately. It does not merge omitted canonical fields or body text.
For T08, assert whole-key metadata replacement before field retention, including the absence of superseded nested keys.
Do not expect a provider-qualified companion to merge. Companion variant support remains outside this change.

T15 does not authorize the deferred routing or owner-schema redesign.
If the accepted stage cannot meet it, report the counterexample to the coordinator and keep readiness unresolved.

For every deliberately invalid fixture, assert the exact relevant finding and affected path.
An import error, missing helper, or check that rejects all inputs is a checker failure.
Use known valid and invalid controls to prove that the checker can both accept and reject.
The worker receives exact writable paths. Keep acceptance files outside those paths.
Review any acceptance-file changes separately from implementation fixes.
For future small-model runs, supply the reviewed checker revision from outside the candidate checkout.
Do not claim that an editable test file or a self-reported digest provides tamper resistance.

## Native discovery acceptance

These are separate from the 18 deterministic contract cases.
They require the selected supported harness. Record observed results without converting unavailable evidence into
success.

1. **N01 Workshop context:** Start a fresh Codex session with a disposable copy of the intended workshop selection and
   scope.
2. **N02 Deck context:** Repeat from a disposable deck context with its own selected set and repository boundary.
3. **N03 Duplicate control:** Add a same-name fixture through another applicable root and confirm that discovery
   cannot pass uniqueness.

For N01 and N02, compare every selected skill against harness-reported catalog paths.
Use explicit invocation and read-only companion access on a harmless representative canary skill.
Do not execute real adoption, installation, or publishing workflows as discovery probes.
Retain machine-readable catalog evidence or a supported harness transcript with observed file access.
A correct final answer without path and access evidence does not pass.
If catalog truncation or an unsupported root prevents complete evidence, report that gap.

Record harness build, model route, working directory, repository boundary, relevant configuration digest, and
effective roots.
Record the selected source identities, bundle digests, catalog entries, file-access evidence, and observation time.
Bind the native record to the same deployed snapshot checked by the static verifier.
The record distinguishes static status, native status, and overall acceptance.
Native unavailability preserves static results but prevents an overall ready verdict.

## Execution commands

Run these from the isolated implementation workspace.

```sh
rtk proxy cargo fmt --all --check
rtk proxy cargo clippy --all-targets --all-features -- -D warnings
rtk proxy cargo test --all-features --test codex_skill_identity --test codex_skill_metadata
rtk proxy cargo test --all-features --test skill_layers --test codex_skill_portability
rtk proxy cargo test --all-features --test skill_layer_config
rtk proxy cargo test --all-features
rtk proxy make validate
```

Use direct assertions on parsed doctor findings and bundle records inside the integration target.
The repository ignores `Cargo.lock`; these commands also work from a fresh clone without a generated lockfile.
Keep source and executable bytes unchanged throughout validation; serialize builds and the final test run.
The native check uses the supported harness surface verified at the selected implementation checkpoint.
Do not invent a Codex catalog command or substitute a different harness.
Keep fixture authors and implementation reviewers independent during acceptance.
