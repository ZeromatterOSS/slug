# Phase 35.6a: Test Fixture `.buckconfig` Classification

**Date**: 2026-05-04
**Total fixtures found**: 358
**Source of truth for collect\_ignore**: `tests/conftest.py` lines 36–104

---

## Section 1: Summary Counts

| Bucket | Count | Description |
|--------|------:|-------------|
| A — delete with test | 66 | Test exercises legacy buckconfig-only behaviour being retired |
| B — migrate to MODULE.bazel | 258 | Trivial cells/buildfile/external\_cells config; test exercises orthogonal feature |
| C — keep with rationale | 34 | Non-trivial knobs that survive or need separate migration decision |
| **Total** | **358** | |

---

## Section 2: Bucket A — Delete With Test

**Rationale summary**: fixture belongs to a test that is either in `collect_ignore` (already excluded from CI), exercises a buckconfig knob being retired in Plan 35 (e.g. `[alias]`, `[kuro] starlark_max_callstack_size`, `[deprecated_config]`, `read_root_config`, `create_unhashed_links`, `representative_config_flags`), or exercises Buck2-specific modifier syntax.

### tests/core/audit/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/audit/test_audit_config_data/.buckconfig` | `test_audit_config.py` | Test exists to verify `kuro audit config` reads buckconfig; uses `[foo]`, `[test]` custom sections as test data |
| `tests/core/audit/test_audit_config_data/code/.buckconfig` | `test_audit_config.py` | Subcell fixture for same test |
| `tests/core/audit/test_audit_config_data/source/.buckconfig` | `test_audit_config.py` | Subcell fixture for same test |

### tests/core/build/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/build/test_build_modifiers_data/.buckconfig` | `test_build_modifiers.py` | Buck2-specific `?modifier` target syntax (not Bazel-compatible) |
| `tests/core/build/test_build_modifiers_report_data/.buckconfig` | `test_build_modifiers_report.py` | Buck2-specific modifier syntax |
| `tests/core/build/test_paranoid_data/execution_platforms/.buckconfig` | `test_paranoid.py` | Buck2-specific paranoid execution mode; most tests already in SKIP\_TESTS |
| `tests/core/build/test_target_aliases_data/.buckconfig` | `test_target_aliases.py` | Uses `[alias]` buckconfig section being retired |
| `tests/core/build/test_unhashed_outputs_data/.buckconfig` | `test_unhashed_outputs.py` | Uses `[kuro] create_unhashed_links` knob being retired |

### tests/core/completion/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/completion/test_completion_data/.buckconfig` | `test_completion.py` | In `collect_ignore`: requires `BUCK2_COMPLETION_VERIFY` env var |
| `tests/core/completion/test_completion_data/cell3/.buckconfig` | `test_completion.py` | Subcell of above |
| `tests/core/completion/test_completion_data/dir1/prelude/.buckconfig` | `test_completion.py` | Subcell of above |
| `tests/core/completion/test_completion_data/dir2/cell2a/.buckconfig` | `test_completion.py` | Subcell of above |

### tests/core/configurations/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/configurations/test_exec_modifier_data/.buckconfig` | `test_exec_modifier.py` | In `collect_ignore`: uses Meta-internal `native.constraint` rule |
| `tests/core/configurations/test_select_buckconfig_data/.buckconfig` | `test_select_buckconfig.py` | Test specifically exercises `select()` on buckconfig values |
| `tests/core/configurations/test_unified_constraint_data/.buckconfig` | `test_unified_constraint.py` | In `collect_ignore`: uses Meta-internal `native.constraint` rule |

### tests/core/console/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/console/test_console_data/.buckconfig` | `test_console.py` | In `collect_ignore`: requires `FIXTURES` env var |

### tests/core/ctargets_command/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/ctargets_command/test_ctargets_modifiers_data/.buckconfig` | `test_ctargets_modifiers.py` | Buck2-specific modifier syntax |

### tests/core/explain/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/explain/test_explain_data/.buckconfig` | `test_explain.py` | In `collect_ignore`: requires manifold |

### tests/core/interpreter/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/interpreter/test_callstack_size_data/.buckconfig` | `test_callstack_size.py` | Uses `[kuro] starlark_max_callstack_size` knob being retired |
| `tests/core/interpreter/test_callstack_size_data/bad/.buckconfig` | `test_callstack_size.py` | Subcell of above |
| `tests/core/interpreter/test_callstack_size_data/good/.buckconfig` | `test_callstack_size.py` | Subcell of above |
| `tests/core/interpreter/test_deprecated_config_data/.buckconfig` | `test_deprecated_config.py` | Uses `[deprecated_config]` buckconfig section being retired; also `[unlike]`, `[some]`, `[other]` custom sections |
| `tests/core/interpreter/test_deprecated_config_data/cell/.buckconfig` | `test_deprecated_config.py` | Subcell fixture for same test |
| `tests/core/interpreter/test_peak_allocated_bytes_data/.buckconfig` | `test_peak_allocated_bytes.py` | In `collect_ignore`: Meta-internal peak alloc tracking; uses `[kuro] check_starlark_peak_memory` |
| `tests/core/interpreter/test_peak_allocated_bytes_exceeds_limit_data/.buckconfig` | `test_peak_allocated_bytes_exceeds_limit.py` | In `collect_ignore`: same |
| `tests/core/interpreter/test_prelude_typecheck_data/.buckconfig` | `test_prelude_typecheck.py` | In `collect_ignore`: Meta-internal typecheck infra |
| `tests/core/interpreter/test_read_root_config_data/.buckconfig` | `test_read_root_config.py` | Test specifically exercises `read_root_config()`; uses custom `[unlike]` section as config data |
| `tests/core/interpreter/test_read_root_config_data/other/.buckconfig` | `test_read_root_config.py` | Subcell of above |
| `tests/core/interpreter/test_unstable_typecheck_data/.buckconfig` | `test_unstable_typecheck.py` | In `collect_ignore`: Meta-internal typecheck infra |

### tests/core/io/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/io/test_allow_eden_data/.buckconfig` | `test_allow_eden.py` | io/ directory; exercises Eden-specific IO |
| `tests/core/io/test_compare_providers_data/.buckconfig` | `test_compare_providers.py` | io/ directory; uses `[kuro] digest_algorithms = BLAKE3-KEYED,SHA1` |
| `tests/core/io/test_compare_providers_data/test_large_action_input/.buckconfig` | `test_compare_providers.py` | Subcell of above |
| `tests/core/io/test_edenfs_aba_data/.buckconfig` | `test_edenfs_aba.py` | In `collect_ignore`: requires Eden; uses `[kuro] file_watcher = edenfs` |
| `tests/core/io/test_edenfs_data/.buckconfig` | `test_edenfs.py` | In `collect_ignore`: requires Eden |
| `tests/core/io/test_edenfs_data/subproject/.buckconfig` | `test_edenfs.py` | Subcell of above |
| `tests/core/io/test_eden_mismatched_root_data/subdir/.buckconfig` | `test_eden_mismatched_root.py` | io/ directory; uses `[kuro] allow_eden_io = true` |
| `tests/core/io/test_file_watcher_data/.buckconfig` | `test_file_watcher.py` | In `collect_ignore`: requires Watchman |
| `tests/core/io/test_fs_hash_crawler_data/.buckconfig` | `test_fs_hash_crawler.py` | In `collect_ignore`: requires Buck2 fs\_hash\_crawler; uses `[kuro] file_watcher = fs_hash_crawler` |
| `tests/core/io/test_modify_eden_data/.buckconfig` | `test_modify_eden.py` | io/ directory; Eden-specific |
| `tests/core/io/test_notify_data/.buckconfig` | `test_notify.py` | In `collect_ignore`: requires Buck2.tests.core; uses `[kuro] file_watcher = notify` |
| `tests/core/io/test_watchman_aba_data/.buckconfig` | `test_watchman_aba.py` | In `collect_ignore`: requires Buck2.tests.core; uses `[kuro] file_watcher = watchman` |
| `tests/core/io/test_watchman_data/.buckconfig` | `test_watchman.py` | In `collect_ignore`: requires Buck2.tests.core; uses `[kuro] file_watcher = watchman` |

### tests/core/log/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/log/test_representative_config_flags_data/.buckconfig` | `test_representative_config_flags.py` | Test exercises buckconfig flag representation in build logs |
| `tests/core/log/test_upload_re_logs_data/.buckconfig` | `test_upload_re_logs.py` | In `collect_ignore`: requires manifold |

### tests/core/query/cquery/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/query/cquery/test_cquery_modifiers_data/.buckconfig` | `test_cquery_modifiers.py` | Buck2-specific modifier syntax |

### tests/core/query/uquery/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/query/uquery/test_uquery_data/bxl_simple/.buckconfig` | `test_uquery.py` | In `collect_ignore`: requires manifold |
| `tests/core/query/uquery/test_uquery_data/bxl_simple/special/.buckconfig` | `test_uquery.py` | Subcell of above |
| `tests/core/query/uquery/test_uquery_data/directory_sources/.buckconfig` | `test_uquery.py` | Subcell of above |
| `tests/core/query/uquery/test_uquery_data/oncall/.buckconfig` | `test_uquery.py` | Subcell of above |
| `tests/core/query/uquery/test_uquery_data/set_operators/.buckconfig` | `test_uquery.py` | Subcell of above |
| `tests/core/query/uquery/test_uquery_data/testsof/.buckconfig` | `test_uquery.py` | Subcell of above |

### tests/core/resource_control/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/resource_control/test_action_suspension_data/.buckconfig` | `test_action_suspension.py` | In `collect_ignore`: requires cgroup; uses `[kuro_resource_control]` section |
| `tests/core/resource_control/test_daemon_memory_metrics_data/.buckconfig` | `test_daemon_memory_metrics.py` | In `collect_ignore`: requires cgroup memory metrics |
| `tests/core/resource_control/test_hybrid_execution_resource_control_data/.buckconfig` | `test_hybrid_execution_resource_control.py` | In `collect_ignore`: requires cgroup + RE; uses `[kuro_resource_control]` |
| `tests/core/resource_control/test_instruction_count_data/.buckconfig` | `test_instruction_count.py` | In `collect_ignore`: requires cgroup instruction counting |
| `tests/core/resource_control/test_memory_reporting_data/.buckconfig` | `test_memory_reporting.py` | In `collect_ignore`: requires cgroup; uses `[kuro_resource_control]` |

### tests/core/run/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/run/test_run_modifiers_data/.buckconfig` | `test_run_modifiers.py` | Buck2-specific modifier syntax |

### tests/core/subscribe/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/subscribe/test_subscribe_data/.buckconfig` | `test_subscribe.py` | In `collect_ignore`: requires `BUCK2_EXPECT` env var |

### tests/core/test/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/test/test_listing_data/.buckconfig` | `test_listing.py` | In `collect_ignore`: requires test discovery listing protocol + RE |
| `tests/core/test/test_modifiers_data/.buckconfig` | `test_modifiers.py` | In `collect_ignore`: Buck2-specific `?modifier` target syntax |

### tests/core/trace_io/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/trace_io/test_trace_io_data/.buckconfig` | `test_trace_io.py` | In `collect_ignore`: requires hg VCS |

### tests/core/vpnless/

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/core/vpnless/test_vpnless_data/.buckconfig` | `test_vpnless.py` | In `collect_ignore`: Meta-internal VPN-less feature |

### tests/e2e/ (entire subtree in collect\_ignore)

| Fixture path | Owning test | Reason |
|---|---|---|
| `tests/e2e/buck_config/test_buck_config_log_data/.buckconfig` | `test_buck_config_log.py` | Entire `tests/e2e/` in `collect_ignore` |
| `tests/e2e/configurations/cfg_constructor/test_cfg_modifiers_attr_data/.buckconfig` | `test_cfg_modifiers_attr.py` | Entire `tests/e2e/` in `collect_ignore`; cfg modifier syntax |
| `tests/e2e/configurations/cfg_constructor/test_invoke_cfg_constructors_bad_constraints_data/.buckconfig` | `test_invoke_cfg_constructors_bad_constraints.py` | Entire `tests/e2e/` in `collect_ignore` |
| `tests/e2e/configurations/cfg_constructor/test_invoke_cfg_constructors_data/.buckconfig` | `test_invoke_cfg_constructors.py` | Entire `tests/e2e/` in `collect_ignore` |

---

## Section 3: Bucket B — Migrate to MODULE.bazel

Trivial configs containing only `[cells]`/`[repositories]` + optional `[buildfile]`, `[cell_aliases]`, `[external_cells]`, `[repository_aliases]`, `[project] ignore`, or `[build] execution_platforms`. The test exercises something orthogonal to buckconfig parsing. All 259 paths below are candidates for the bulk migration script in Phase 35.6b.

```
tests/core/analysis/test_analysis_action_ids_unique_data/category/.buckconfig
tests/core/analysis/test_analysis_action_ids_unique_data/identifier/.buckconfig
tests/core/analysis/test_analysis_queries_data/analysis_query_deps/.buckconfig
tests/core/analysis/test_analysis_queries_data/analysis_query_invalidation/.buckconfig
tests/core/analysis/test_analysis_queries_data/analysis_query_invalidation_classpath/.buckconfig
tests/core/analysis/test_cmd_args_data/.buckconfig
tests/core/analysis/test_depset_order_data/.buckconfig
tests/core/analysis/test_runfiles_data/.buckconfig
tests/core/analysis/test_template_placeholder_data/.buckconfig
tests/core/audit/test_audit_cells_data/.buckconfig
tests/core/audit/test_audit_common_opts_data/.buckconfig
tests/core/audit/test_audit_configurations_data/.buckconfig
tests/core/audit/test_audit_execution_platform_resolution_data/.buckconfig
tests/core/audit/test_audit_file_package_data/.buckconfig
tests/core/audit/test_audit_file_package_data/newcell/.buckconfig
tests/core/audit/test_audit_includes_data/.buckconfig
tests/core/audit/test_audit_output_data/.buckconfig
tests/core/audit/test_audit_output_data/cell1/.buckconfig
tests/core/audit/test_audit_parse_data/.buckconfig
tests/core/audit/test_audit_providers_data/modifiers/.buckconfig
tests/core/audit/test_audit_providers_data/sorted/.buckconfig
tests/core/audit/test_audit_providers_data/universe/.buckconfig
tests/core/audit/test_audit_subtargets_data/.buckconfig
tests/core/audit/test_audit_visibility_data/.buckconfig
tests/core/build/actions/test_actions_data/actions/.buckconfig
tests/core/build/actions/test_copy_data/.buckconfig
tests/core/build/actions/test_dynamic_output_data/artifact_eq_bug/.buckconfig
tests/core/build/actions/test_dynamic_output_data/empty_dynamic_list/.buckconfig
tests/core/build/actions/test_dynamic_output_data/everything/.buckconfig
tests/core/build/actions/test_dynamic_output_data/everything_new/.buckconfig
tests/core/build/actions/test_dynamic_value_data/.buckconfig
tests/core/build/actions/test_output_artifact_twice_data/.buckconfig
tests/core/build/actions/test_projected_output_artifact_data/.buckconfig
tests/core/build/actions/test_unbound_artifact_data/.buckconfig
tests/core/build/actions/test_write_data/write/.buckconfig
tests/core/build/actions/test_write_data/write_fails/.buckconfig
tests/core/build/macros/test_macros_data/.buckconfig
tests/core/build/macros/test_write_to_file_macros_data/.buckconfig
tests/core/build/test_action_error_handler_types_data/.buckconfig
tests/core/build/test_build_configured_data/.buckconfig
tests/core/build/test_build_output_data/.buckconfig
tests/core/build/test_build_report_data/.buckconfig
tests/core/build/test_build_report_errors_data/.buckconfig
tests/core/build/test_build_response_data/.buckconfig
tests/core/build/test_build_root_executable_data/.buckconfig
tests/core/build/test_build_rule_type_name_logging_data/.buckconfig
tests/core/build/test_build_skip_incompatible_targets_data/.buckconfig
tests/core/build/test_build_system_info_data/.buckconfig
tests/core/build/test_build_with_transition_data/.buckconfig
tests/core/build/test_critical_path_data/.buckconfig
tests/core/build/test_detailed_aggregated_metrics_data/.buckconfig
tests/core/build/test_error_categorization_data/.buckconfig
tests/core/build/test_external_buckconfigs_data/.buckconfig
tests/core/build/test_modify_data/modify/.buckconfig
tests/core/build/test_nested_subtargets_data/.buckconfig
tests/core/build/test_out_flag_data/.buckconfig
tests/core/build/test_overall_timeout_data/.buckconfig
tests/core/build/test_plugins_data/.buckconfig
tests/core/build/test_skip_missing_data/.buckconfig
tests/core/build/test_symlinks_data/.buckconfig
tests/core/build/test_uncategorized_data/action_digest/.buckconfig
tests/core/build/test_uncategorized_data/anon_exec_deps/.buckconfig
tests/core/build/test_uncategorized_data/args/.buckconfig
tests/core/build/test_uncategorized_data/buckroot/rooted/.buckconfig
tests/core/build/test_uncategorized_data/buckroot/rooted/cell/.buckconfig
tests/core/build/test_uncategorized_data/build_providers/.buckconfig
tests/core/build/test_uncategorized_data/cell_delete/.buckconfig
tests/core/build/test_uncategorized_data/cleanup/.buckconfig
tests/core/build/test_uncategorized_data/concurrency/.buckconfig
tests/core/build/test_uncategorized_data/fail_fast/.buckconfig
tests/core/build/test_uncategorized_data/invalid_file_invalidation/.buckconfig
tests/core/build/test_uncategorized_data/keep_going_build/.buckconfig
tests/core/build/test_uncategorized_data/log_action_keys/.buckconfig
tests/core/build/test_uncategorized_data/roots/.buckconfig
tests/core/build/test_uncategorized_data/roots/other/.buckconfig
tests/core/build/test_uncategorized_data/tmpdir/.buckconfig
tests/core/build/test_uncategorized_data/upload_all_actions/.buckconfig
tests/core/build/test_universe_data/.buckconfig
tests/core/bxl/test_analysis_data/.buckconfig
tests/core/bxl/test_anon_bxl_data/.buckconfig
tests/core/bxl/test_audit_data/.buckconfig
tests/core/bxl/test_cli_data/.buckconfig
tests/core/bxl/test_configured_target_data/.buckconfig
tests/core/bxl/test_dynamic_new_data/.buckconfig
tests/core/bxl/test_ensure_data/.buckconfig
tests/core/bxl/test_execution_platforms_data/.buckconfig
tests/core/bxl/test_lazy_build_artifact_data/.buckconfig
tests/core/bxl/test_lazy_uquery_data/.buckconfig
tests/core/bxl/test_not_bxl_data/.buckconfig
tests/core/bxl/test_output_data/.buckconfig
tests/core/bxl/test_package_data/.buckconfig
tests/core/bxl/test_selector_concat_dict_data/.buckconfig
tests/core/bxl/test_streaming_data/.buckconfig
tests/core/bxl/test_target_universe_data/.buckconfig
tests/core/bxl/test_target_universe_data/some_cell/.buckconfig
tests/core/bxl/test_typecheck_data/.buckconfig
tests/core/bxl/test_type_names_and_symbols_data/.buckconfig
tests/core/bxl/test_unconfigured_target_nodes_keep_going_data/.buckconfig
tests/core/bzlmod/test_bazel_dep_data/.buckconfig
tests/core/bzlmod/test_local_path_override_data/.buckconfig
tests/core/bzlmod/test_local_path_override_data/libs/local_lib/.buckconfig
tests/core/bzlmod/test_module_directive_data/.buckconfig
tests/core/bzlmod/test_module_parsing_data/.buckconfig
tests/core/bzlmod/test_multi_module_project_data/.buckconfig
tests/core/bzlmod/test_multi_module_project_data/libs/lib_a/.buckconfig
tests/core/bzlmod/test_multi_module_project_data/libs/lib_b/.buckconfig
tests/core/clean/test_clean_data/.buckconfig
tests/core/client/test_common_opts_data/.buckconfig
tests/core/completion/test_complete_command_data/.buckconfig
tests/core/completion/test_complete_command_data/cell1/.buckconfig
tests/core/completion/test_complete_command_data/cell1/buck2/fake_prelude/.buckconfig
tests/core/completion/test_complete_command_data/cell2/.buckconfig
tests/core/completion/test_complete_command_data/dir3/cell3a/.buckconfig
tests/core/completion/test_complete_command_data/dir3/cell3b/.buckconfig
tests/core/configurations/test_configuration_dep_uquery_correctness_data/.buckconfig
tests/core/configurations/test_configuration_rule_unbound_data/.buckconfig
tests/core/configurations/test_evaluation_order_data/.buckconfig
tests/core/configurations/test_extra_exec_platforms_data/.buckconfig
tests/core/configurations/test_nested_select_coercion_data/.buckconfig
tests/core/configurations/test_per_exec_group_platforms_data/.buckconfig
tests/core/configurations/test_platform_via_alias_data/.buckconfig
tests/core/configurations/test_platform_wrong_label_data/.buckconfig
tests/core/configurations/test_select_concat_data/.buckconfig
tests/core/configurations/test_select_refine_data/.buckconfig
tests/core/configurations/test_subtarget_configuration_dep_data/.buckconfig
tests/core/configurations/test_target_platforms_arg_data/.buckconfig
tests/core/configurations/test_target_platforms_arg_data/subcell/.buckconfig
tests/core/configurations/test_toolchain_overconfiguration_data/.buckconfig
tests/core/configurations/test_unknown_exec_group_data/.buckconfig
tests/core/configurations/transition/test_access_attr_data/.buckconfig
tests/core/configurations/transition/test_attr_data/.buckconfig
tests/core/configurations/transition/test_attr_split_data/.buckconfig
tests/core/configurations/transition/test_constructor_validation_data/.buckconfig
tests/core/configurations/transition/test_rule_data/.buckconfig
tests/core/configurations/transition/test_rule_infinite_bug_data/.buckconfig
tests/core/configurations/transition/test_select_in_transition_attr_data/.buckconfig
tests/core/configurations/transition/test_toolchain_incoming_data/.buckconfig
tests/core/configurations/transition/test_transition_info_data/.buckconfig
tests/core/console/test_emit_console_preferences_data/.buckconfig
tests/core/ctargets_command/test_ctargets_basic_data/.buckconfig
tests/core/ctargets_command/test_ctargets_incompatible_data/.buckconfig
tests/core/ctargets_command/test_ctargets_json_report_data/.buckconfig
tests/core/ctargets_command/test_ctargets_keep_going_data/.buckconfig
tests/core/ctargets_command/test_ctargets_skip_missing_targets_data/.buckconfig
tests/core/ctargets_command/test_ctargets_transition_data/.buckconfig
tests/core/daemon/test_concurrency_data/.buckconfig
tests/core/daemon/test_daemon_buster_data/.buckconfig
tests/core/daemon/test_daemon_data/.buckconfig
tests/core/daemon/test_daemon_tokio_metrics_data/.buckconfig
tests/core/daemon/test_nested_invocations_data/.buckconfig
tests/core/debug/test_debug_chrome_trace_data/.buckconfig
tests/core/debug/test_debug_data/.buckconfig
tests/core/debug/test_debug_eval_data/.buckconfig
tests/core/dice_dump/test_dump_data/.buckconfig
tests/core/docs/test_builtin_docs_data/.buckconfig
tests/core/docs/test_docs_data/.buckconfig
tests/core/docs/test_docs_data/cell/.buckconfig
tests/core/errors/test_command_report_data/.buckconfig
tests/core/errors/test_command_report_data/empty_buckconfig/.buckconfig
tests/core/errors/test_exit_code_data/.buckconfig
tests/core/errors/test_formatting_data/.buckconfig
tests/core/executor/test_build_id_env_var_data/.buckconfig
tests/core/executor/test_cache_uploads_data/.buckconfig
tests/core/executor/test_cancellation_data/.buckconfig
tests/core/executor/test_dep_files_data/upload_dep_files/.buckconfig
tests/core/executor/test_executor_with_dependencies_data/.buckconfig
tests/core/executor/test_executor_with_re_gang_workers_data/.buckconfig
tests/core/executor/test_hybrid_executor_data/.buckconfig
tests/core/executor/test_incremental_actions_data/.buckconfig
tests/core/executor/test_materialization_for_failed_actions_data/materialize_inputs_for_failed_actions/.buckconfig
tests/core/executor/test_materialization_for_failed_actions_data/materialize_outputs_for_failed_actions/.buckconfig
tests/core/executor/test_no_executor_data/.buckconfig
tests/core/executor/test_output_cleanup_data/.buckconfig
tests/core/executor/test_outputs_ordering_data/.buckconfig
tests/core/executor/test_remote_execution_data/.buckconfig
tests/core/external_cells/test_in_subdir_data/.buckconfig
tests/core/external_cells/test_prelude_data/.buckconfig
tests/core/help/test_help_data/.buckconfig
tests/core/help/test_help_env_data/.buckconfig
tests/core/http2/test_http2_data/.buckconfig
tests/core/incremental_api/test_incremental_remote_action_data/.buckconfig
tests/core/interpreter/test_attr_default_coercion_data/.buckconfig
tests/core/interpreter/test_cancellation_data/.buckconfig
tests/core/interpreter/test_cpu_instruction_count_data/.buckconfig
tests/core/interpreter/test_load_json_data/.buckconfig
tests/core/interpreter/test_missing_source_file_data/.buckconfig
tests/core/interpreter/test_package_file_alt_name_data/.buckconfig
tests/core/interpreter/test_package_file_package_values_data/.buckconfig
tests/core/interpreter/test_package_file_visibility_data/.buckconfig
tests/core/interpreter/test_package_values_cross_cell_data/.buckconfig
tests/core/interpreter/test_package_values_cross_cell_data/other/.buckconfig
tests/core/interpreter/test_package_values_missing_buck_file_data/.buckconfig
tests/core/interpreter/test_print_data/.buckconfig
tests/core/interpreter/test_rule_implementation_data/.buckconfig
tests/core/interpreter/test_sub_packages_data/.buckconfig
tests/core/interpreter/test_v2_only_data/.buckconfig
tests/core/invalidation/test_forward_node_data/.buckconfig
tests/core/invalidation/test_ignored_directory_entry_data/.buckconfig
tests/core/invalidation/test_root_directory_data/.buckconfig
tests/core/kill/test_kill_data/.buckconfig
tests/core/log/test_diff_data/.buckconfig
tests/core/log/test_log_data/.buckconfig
tests/core/log/test_replay_data/.buckconfig
tests/core/log/test_summary_data/.buckconfig
tests/core/log/test_user_event_log_data/.buckconfig
tests/core/log/test_what_materialized_data/.buckconfig
tests/core/log/test_what_ran_incomplete_data/.buckconfig
tests/core/log/test_whatup_data/.buckconfig
tests/core/log/test_what_uploaded_data/.buckconfig
tests/core/lsp/test_lsp_data/.buckconfig
tests/core/lsp/test_lsp_data/cell/.buckconfig
tests/core/lsp/test_lsp_data/prelude/.buckconfig
tests/core/profile/test_profile_data/.buckconfig
tests/core/query/aquery/test_aquery_data/.buckconfig
tests/core/query/cquery/test_compatible_with_data/.buckconfig
tests/core/query/cquery/test_cquery_data/deps_query/.buckconfig
tests/core/query/cquery/test_cquery_data/multi_query_universe/.buckconfig
tests/core/query/cquery/test_cquery_data/set_operators/.buckconfig
tests/core/query/cquery/test_cquery_data/testsof/.buckconfig
tests/core/query/cquery/test_cquery_data/toolchain_deps/.buckconfig
tests/core/query/cquery/test_cquery_data/unsorted/.buckconfig
tests/core/query/cquery/test_cquery_data/unsorted/special/.buckconfig
tests/core/query/cquery/test_cquery_data/visibility/.buckconfig
tests/core/query/cquery/test_cquery_with_transition_data/.buckconfig
tests/core/query/cquery/test_filter_data/.buckconfig
tests/core/query/cquery/test_owner_data/deprecated_correct/.buckconfig
tests/core/query/cquery/test_owner_data/incompatible/.buckconfig
tests/core/query/cquery/test_owner_isolated_data/simple/.buckconfig
tests/core/query/test_buildfiles_data/.buckconfig
tests/core/query/test_target_call_stacks_data/.buckconfig
tests/core/query/test_target_configuration_toolchain_deps_traversal_data/.buckconfig
tests/core/rage/test_rage_data/.buckconfig
tests/core/run/test_build_id_env_data/.buckconfig
tests/core/run/test_run_data/.buckconfig
tests/core/run/test_universe_data/.buckconfig
tests/core/starlark_command/test_lint_and_typecheck_data/.buckconfig
tests/core/target_graph/test_visibility_from_package_data/.buckconfig
tests/core/target_graph/test_within_view_data/.buckconfig
tests/core/targets_command/test_call_stacks_data/.buckconfig
tests/core/targets_command/test_content_based_paths_data/.buckconfig
tests/core/targets_command/test_recursive_data/.buckconfig
tests/core/targets_command/test_skip_targets_with_duplicate_names_data/.buckconfig
tests/core/targets_command/test_target_hashing_data/.buckconfig
tests/core/targets_command/test_target_metadata_data/.buckconfig
tests/core/targets_command/test_targets_imports_data/.buckconfig
tests/core/targets_command/test_targets_keep_going_data/.buckconfig
tests/core/targets_command/test_unconfigured_target_hashing_data/.buckconfig
tests/core/test/test_build_report_data/.buckconfig
tests/core/test/test_internal_runner_data/.buckconfig
tests/core/test/test_local_resources_data/.buckconfig
tests/core/test/test_platform_resolution_data/.buckconfig
tests/core/test/test_selection_data/.buckconfig
tests/core/test/test_skip_incompatible_targets_data/.buckconfig
tests/core/test/test_startup_data/.buckconfig
tests/core/test/test_test_rule_type_name_logging_data/.buckconfig
tests/core/transitive_sets/test_transitive_sets_data/.buckconfig
tests/core/validation/test_concurrent_validation_data/.buckconfig
tests/core/validation/test_target_validation_data/.buckconfig
```

**Notes on non-obvious bucket-B inclusions**:

- `test_external_buckconfigs_data` — the fixture itself is trivial `[cells]+[cell_aliases]+[external_cells]+[buildfile]`; all 4 tests pass per conftest comments (the test exercises the `--config-file` flag mechanism, not the fixture's own buckconfig).
- `test_build_modify_data/modify` — minimal `[cells]+[buildfile]` only; the actual modify test with `[kuro] materializations = deferred` is in bucket C (`modify_file_during_build`).
- `test_docs_data` — large multi-cell setup but all sections are `[repositories]`, `[project] ignore`, `[buildfile]` — no legacy knobs.
- `test_complete_command_data` (not `test_completion_data`) — the test is NOT in collect\_ignore (only `test_completion.py` is); the complete\_command test exercises tab completion command logic, not buckconfig parsing.
- `test_audit_cells_data` — uses `[cells]` + `[cell_aliases]` only; the test exercises `audit cells` output, not buckconfig reading per se.
- Files with `[build] execution_platforms = <target>` — this maps to `--build_event_service` / execution platform selection via Starlark; the section key is a legacy buckconfig path but the value is a build target label. However since this knob has a direct Bazel-compat equivalent (`--build` flag or `.bazelrc` line), it qualifies as bucket B (the per-fixture migration script converts `[build] execution_platforms` to a `.bazelrc` `build --platforms=...` line).

---

## Section 4: Bucket C — Keep With Rationale

These 33 fixtures use buckconfig keys that are either: (a) not yet migrated to a CLI flag in Plan 35.5, (b) test data for `read_config()` / `read_root_config()` calls in the fixture's BUILD rules, or (c) intentionally broken/unusual fixture structure that cannot be expressed in MODULE.bazel.

| Fixture path | Owning test | Special key(s) | Rationale |
|---|---|---|---|
| `tests/core/audit/test_audit_deferred_materializer_data/.buckconfig` | `test_audit_deferred_materializer.py` | `[kuro] materializations = deferred` | Test specifically exercises deferred materializer audit command; knob not yet promoted to CLI flag |
| `tests/core/build/test_uncategorized_data/artifact_consistency/.buckconfig` | `test_uncategorized.py` | `[kuro] allow_eden_io = false`, `[kuro] dice = modern` | Two `[kuro]` knobs still in flux; `dice = modern` controls DICE mode selection, not yet a CLI flag |
| `tests/core/build/test_uncategorized_data/buckroot/.buckconfig` | `test_uncategorized.py::test_buckroot` | intentionally malformed `[ BROKEN` header | Fixture must be non-parseable; tests that `.buckroot` file overrides `.buckconfig` for workspace root detection — cannot be expressed as MODULE.bazel |
| `tests/core/build/test_uncategorized_data/prelude_import/.buckconfig` | `test_uncategorized.py::test_prelude_imported_once` | `[project] ignore` | Multi-cell with ignore; OK, but subcells (below) are C |
| `tests/core/build/test_uncategorized_data/prelude_import/cell1/.buckconfig` | `test_uncategorized.py::test_prelude_imported_once` | `[test] config = cell1` | `[test]` section is buckconfig data read by `read_config("test","config")` in prelude.bzl; the test verifies prelude reads its own cell's config, not another cell's |
| `tests/core/build/test_uncategorized_data/prelude_import/cell2/.buckconfig` | `test_uncategorized.py::test_prelude_imported_once` | `[test] config = cell2` | Same; subcell for above |
| `tests/core/build/test_uncategorized_data/prelude_import/prelude/.buckconfig` | `test_uncategorized.py::test_prelude_imported_once` | `[test] config = prelude` | Same; prelude cell buckconfig data consumed by `read_config()` in rule implementation |
| `tests/core/build/test_uncategorized_data/projected_artifacts/.buckconfig` | `test_uncategorized.py` | `[kuro] materializations = deferred` | Deferred materialization knob not yet promoted to CLI flag |
| `tests/core/bxl/test_actions_data/.buckconfig` | `test_actions.py` | `[kuro] materializations=deferred`, `enable_local_caching_of_re_artifacts`, `sqlite_materializer_state*`, `defer_write_actions` | Five `[kuro]` RE/materializer knobs not yet CLI flags; test exercises BXL action execution with deferred materializer |
| `tests/core/bxl/test_build_data/.buckconfig` | `test_build.py` | `[kuro] materializations = deferred` | Deferred materializer knob |
| `tests/core/bxl/test_dynamic_data/.buckconfig` | `test_dynamic.py` | `[kuro] materializations=deferred`, `enable_local_caching_of_re_artifacts`, `sqlite_materializer_state*`, `defer_write_actions` | Same RE/materializer cluster as test\_actions |
| `tests/core/client/test_argfiles_data/.buckconfig` | `test_argfiles.py` | `[foo] bar = 0` | Custom `[foo]` section is baseline buckconfig data that argfile tests override via `--config=foo.bar=1`; the test verifies argfile override semantics; section name is opaque to the parser but the test logic depends on this value |
| `tests/core/configurations/test_target_incompatible_data/.buckconfig` | `test_target_incompatible.py` | `[kuro] error_on_dep_only_incompatible = <targets>` | Knob controls incompatible-target error behaviour; sets a multi-line target list that the test verifies causes specific errors; not yet a CLI flag |
| `tests/core/cycle_detection/test_cycle_detection_data/.buckconfig` | `test_cycle_detection.py` | `[build] lazy_cycle_detector = true`, `[kuro] detect_cycles = disabled` | Two non-standard knobs: `lazy_cycle_detector` is a `[build]` key not in standard Bazel; `detect_cycles = disabled` is a `[kuro]` knob; test exercises cycle detection behaviour variants |
| `tests/core/digest/test_digest_data/.buckconfig` | `test_digest.py` | `[kuro] digest_algorithms = BLAKE3-KEYED,SHA1` | `digest_algorithms` selects hash algorithm; not yet a CLI flag; test specifically validates BLAKE3-KEYED digest behaviour |
| `tests/core/errors/test_errors_data/.buckconfig` | `test_errors.py` | `[kuro] allow_eden_io = false`, `[project] ignore = package_listing/*red/**` | `allow_eden_io` is a `[kuro]` knob; `project.ignore` with a regex glob pattern is not MODULE.bazel `.bazelignore` syntax (`.bazelignore` uses simple prefixes, not glob patterns with `/`) |
| `tests/core/executor/test_content_based_paths_data/.buckconfig` | `test_content_based_paths.py` | `[kuro] create_unhashed_links = true` | Same `create_unhashed_links` knob as test\_unhashed\_outputs; but this test is NOT bucket A because it tests executor content-based paths, not the unhashed-outputs feature itself — the knob is incidental scaffolding |
| `tests/core/executor/test_dep_files_data/dep_files/.buckconfig` | `test_dep_files.py` | `[kuro] materializations = deferred` | Deferred materializer knob |
| `tests/core/executor/test_dep_files_data/invalid_dep_files/.buckconfig` | `test_dep_files.py` | `[kuro] materializations = deferred` | Same |
| `tests/core/executor/test_dep_files_data/mismatched_outputs_dep_files/.buckconfig` | `test_dep_files.py` | `[kuro] materializations=deferred`, `[kuro] hash_all_commands=true` | Two `[kuro]` knobs |
| `tests/core/executor/test_hash_all_commands_data/.buckconfig` | `test_hash_all_commands.py` | `[kuro] materializations=deferred`, `hash_all_commands=true`, `declare_match_in_depfiles=true`, `declare_in_local_executor=true` | Four `[kuro]` knobs related to dep-file hashing behaviour; test exercises hash\_all\_commands feature |
| `tests/core/interpreter/test_relative_paths_data/.buckconfig` | `test_relative_paths.py` | `[kuro] directories_to_allow_relative_paths = //foo` | Knob controls which directories permit relative-path imports; not yet a CLI flag |
| `tests/core/invocation_record/test_build_count_data/.buckconfig` | `test_build_count.py` | `[kuro] file_watcher = edenfs`, `[project] watchman_merge_base = main` | `file_watcher` and `watchman_merge_base` are io-subsystem knobs; this test is not in collect\_ignore (it records build counts, not Eden-specific behaviour), but these knobs are not yet CLI flags |
| `tests/core/invocation_record/test_invocation_record_data/.buckconfig` | `test_invocation_record.py` | `[kuro] materializations = deferred` | Deferred materializer knob |
| `tests/core/materializer/test_clean_stale_bxl_data/.buckconfig` | `test_clean_stale_bxl.py` | `[kuro] materializations=deferred`, `enable_local_caching_of_re_artifacts`, `sqlite_materializer_state*`, `defer_write_actions` | RE/materializer cluster knobs |
| `tests/core/materializer/test_clean_stale_data/.buckconfig` | `test_clean_stale.py` | `[kuro]` full RE cluster + `update_access_times = full` | `update_access_times` is an additional knob not yet in CLI |
| `tests/core/materializer/test_materializer_data/deferred_materializer_matching_artifact_optimization/.buckconfig` | `test_materializer.py` | `[kuro] materializations=deferred`, RE cluster (4 knobs) | Materializer RE knobs |
| `tests/core/materializer/test_materializer_data/modify_deferred_materialization/.buckconfig` | `test_materializer.py` | `[kuro] materializations = deferred` | Deferred materializer knob |
| `tests/core/materializer/test_materializer_data/modify_deferred_materialization_deps/.buckconfig` | `test_materializer.py` | `[kuro] materializations = deferred` | Deferred materializer knob |
| `tests/core/materializer/test_symlink_local_remote_bug_data/.buckconfig` | `test_symlink_local_remote_bug.py` | `[kuro] materializations = deferred` | Deferred materializer knob |
| `tests/core/materializer/test_symlink_to_parent_bug_data/.buckconfig` | `test_symlink_to_parent_bug.py` | `[kuro] materializations = deferred` | Deferred materializer knob |
| `tests/core/restart/test_restart_data/.buckconfig` | `test_restart.py` | `[kuro] materializations=deferred`, `sqlite_materializer_state*`, `restarter=true` | Three `[kuro]` knobs; `restarter=true` is an active restart-on-state-change knob not yet a CLI flag |
| `tests/core/test/test_content_based_paths_data/.buckconfig` | `test_content_based_paths.py` | `[kuro] create_unhashed_links = true` | Same as executor/test\_content\_based\_paths; knob is incidental scaffolding, not the feature under test |
| `tests/core/build/test_modify_data/modify_file_during_build/.buckconfig` | `test_modify.py` | `[kuro] materializations = deferred` | Deferred materializer knob; test exercises file-change detection during an active build |

---

*Sanity check: 66 + 258 + 34 = 358.*

## Section 5: Knob Frequency Table

All `(section, key)` pairs across all 358 fixtures, descending by count.

| Count | Section | Key | Notes |
|------:|---------|-----|-------|
| 136 | `[buildfile]` | `name` | Build file name override (TARGETS.fixture / BUILD.bazel / OUTER / INNER) |
| 97 | `[repositories]` | `prelude` | Legacy cell declaration (old-style) |
| 95 | `[repositories]` | `root` | Legacy cell declaration (old-style) |
| 41 | `[project]` | `ignore` | Ignore glob patterns |
| 28 | `[cells]` | `root` | Modern cell declaration |
| 22 | `[cell_aliases]` | `prelude` | Prelude alias |
| 21 | `[external_cells]` | `nano_prelude` | Bundled nano\_prelude |
| 17 | `[cells]` | `nano_prelude` | Bundled nano\_prelude cell |
| 13 | `[external_cells]` | `prelude` | Bundled prelude |
| 12 | `[cell_aliases]` | `toolchains` | Toolchains cell alias |
| 12 | `[cell_aliases]` | `ovr_config` | Meta-internal ovr\_config alias |
| 12 | `[cell_aliases]` | `fbsource` | Meta-internal fbsource alias |
| 12 | `[cell_aliases]` | `fbcode` | Meta-internal fbcode alias |
| 10 | `[repositories]` | `config` | Legacy config cell |
| 8 | `[repositories]` | `cell` | Legacy generic cell |
| 6 | `[build]` | `execution_platforms` | Execution platform target label |
| 6 | `[cell_aliases]` | `config` | Config cell alias |
| 6 | `[cell_aliases]` | `buck` | Buck cell alias |
| 5 | `[kuro]` | `materializations` | Deferred/eager materializer mode |
| 5 | `[repositories]` | `buck` | Legacy buck cell |
| 5 | `[project]` | `package_boundary_exceptions` | Package boundary override |
| 4 | `[repositories]` | `nano_prelude` | Legacy nano\_prelude cell |
| 4 | `[cells]` | `prelude` | Modern prelude cell |
| 3 | `[test]` | `config` | Test data for `read_config("test","config")` in prelude\_import fixture |
| 3 | `[repositories]` | `toolchains` | Legacy toolchains cell |
| 3 | `[repositories]` | `special` | Legacy special cell |
| 3 | `[repositories]` | `fbsource` | Legacy fbsource cell |
| 3 | `[repositories]` | `fbcode` | Legacy fbcode cell |
| 3 | `[cells]` | `toolchains` | Modern toolchains cell |
| 3 | `[cells]` | `fbsource` | Modern fbsource cell |
| 3 | `[cells]` | `fbcode` | Modern fbcode cell |
| 3 | `[cells]` | `config` | Modern config cell |
| 3 | `[cells]` | `buck` | Modern buck cell |
| 2 | `[kuro]` | `sqlite_materializer_state_version` | Materializer SQLite state version |
| 2 | `[kuro]` | `sqlite_materializer_state` | Materializer SQLite state toggle |
| 2 | `[kuro]` | `enable_local_caching_of_re_artifacts` | Local RE artifact cache |
| 2 | `[kuro]` | `defer_write_actions` | Defer write actions in materializer |
| 2 | `[kuro]` | `check_starlark_peak_memory` | Peak memory tracking (bucket A only) |
| 2 | `[kuro]` | `allow_eden_io` | Eden IO permission toggle |
| 2 | `[cells]` | `special` | Special cell |
| 2 | `[cells]` | `local_lib` | Local library cell |
| 2 | `[cells]` | `lib_b` | Library B cell |
| 2 | `[cells]` | `lib_a` | Library A cell |
| 1 | `[kuro]` | `dice` | DICE mode (modern vs legacy) |
| 1 | `[kuro]` | `file_watcher` | File watcher backend (edenfs/watchman/notify/fs\_hash\_crawler) |
| 1 | `[kuro]` | `starlark_max_callstack_size` | Max callstack depth (bucket A — retiring) |
| 1 | `[kuro]` | `create_unhashed_links` | Unhashed symlink creation |
| 1 | `[kuro]` | `error_on_dep_only_incompatible` | Error on dep-only incompatible targets |
| 1 | `[kuro]` | `detect_cycles` | Cycle detection mode |
| 1 | `[kuro]` | `digest_algorithms` | Hash algorithm selection (BLAKE3-KEYED,SHA1) |
| 1 | `[kuro]` | `directories_to_allow_relative_paths` | Allow relative .bzl imports in dirs |
| 1 | `[kuro]` | `hash_all_commands` | Hash all action commands for dep files |
| 1 | `[kuro]` | `declare_match_in_depfiles` | Dep file declaration matching |
| 1 | `[kuro]` | `declare_in_local_executor` | Dep file declaration in local executor |
| 1 | `[kuro]` | `restarter` | Restart daemon on state change |
| 1 | `[kuro]` | `update_access_times` | Access time update mode |
| 1 | `[build]` | `lazy_cycle_detector` | Lazy cycle detection (not standard Bazel) |
| 1 | `[build]` | `threads` | Thread count (resource\_control, bucket A) |
| 1 | `[kuro_resource_control]` | `status` | Resource control status |
| 1 | `[kuro_resource_control]` | `enable_suspension` | Cgroup suspension |
| 1 | `[kuro_resource_control]` | `memory_high_action_cgroup_pool` | Cgroup memory high limit |
| 1 | `[kuro_resource_control]` | `enable_action_cgroup_pool_v2` | Action cgroup pool v2 |
| 1 | `[deprecated_config]` | `some.config1` | Deprecated config deprecation notice (bucket A) |
| 1 | `[deprecated_config]` | `other.config1` | Deprecated config deprecation notice (bucket A) |
| 1 | `[deprecated_config]` | `other.config2` | Same, subcell (bucket A) |
| 1 | `[alias]` | `alias` | Target alias (bucket A — retiring) |
| 1 | `[alias]` | `chain` | Chained alias (bucket A — retiring) |
| 1 | `[alias]` | `bad` | Bad alias (bucket A — retiring) |
| 1 | `[alias]` | `other_alias` | Target alias in explain test (bucket A) |
| 1 | `[foo]` | `a` | Custom test-data section in audit\_config (bucket A) |
| 1 | `[foo]` | `b` | Custom test-data section in audit\_config (bucket A) |
| 1 | `[foo]` | `bar` | Custom test-data section in argfiles (bucket C) |
| 1 | `[bar]` | `a` | Custom test-data section in audit\_config (bucket A) |
| 1 | `[test]` | `is_root` | Custom test-data in audit\_config (bucket A) |
| 1 | `[test]` | `is_code` | Custom test-data in audit\_config (bucket A) |
| 1 | `[unlike]` | `harsh` | Custom test-data for read\_root\_config (bucket A) |
| 1 | `[some]` | `config1` | Custom test-data for deprecated\_config (bucket A) |
| 1 | `[other]` | `config1` | Custom test-data for deprecated\_config (bucket A) |
| 1 | `[other]` | `config2` | Custom test-data for deprecated\_config (bucket A) |
| 1 | `[other]` | `config3` | Custom test-data for deprecated\_config (bucket A) |
| 1 | `[project]` | `watchman_merge_base` | Watchman merge base (io/* tests mostly, bucket A) |

---

*End of classification. Sanity check: 66 + 258 + 34 = 358. No fixture appears in more than one bucket.*
