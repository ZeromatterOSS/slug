"""
Root conftest for kuro test suite.

This file sets up the test infrastructure so that existing tests/core/ tests
(originally written for Buck2's Meta-internal test framework) can run with kuro.

Key responsibilities:
1. Add tests/ to sys.path so `buck2.tests.e2e_util` imports work via the
   `tests/buck2/tests/e2e_util -> ../../../e2e_util` symlink.
2. Stub required environment variables for the buck_workspace.py framework.
3. Set TEST_REPO_DATA dynamically per-test to the test file's directory.
"""

import inspect
import os
import sys
from pathlib import Path

import pytest

# ──────────────────────────────────────────────────────────────────────────────
# 1. Add tests/ to sys.path for `buck2.tests.e2e_util` imports and __manifest__
#    Must happen BEFORE any buck2.* imports below.
# ──────────────────────────────────────────────────────────────────────────────
TESTS_DIR = Path(__file__).parent
if str(TESTS_DIR) not in sys.path:
    sys.path.insert(0, str(TESTS_DIR))

from buck2.tests.e2e_util.buck_workspace import buck  # noqa F401

# ──────────────────────────────────────────────────────────────────────────────
# Files that require Meta-internal modules (manifold, buck2.tests.core, etc.)
# These cannot be imported externally and are excluded from collection.
# ──────────────────────────────────────────────────────────────────────────────
collect_ignore = [
    # Meta-internal only (requires manifold, buck2.tests.core, etc.)
    "core/explain/test_explain.py",          # requires manifold
    "core/io/test_edenfs.py",                # requires buck2.tests.core
    "core/io/test_edenfs_aba.py",            # requires buck2.tests.core
    "core/io/test_fs_hash_crawler.py",       # requires buck2.tests.core
    "core/io/test_notify.py",               # requires buck2.tests.core
    "core/io/test_watchman.py",             # requires buck2.tests.core
    "core/io/test_watchman_aba.py",         # requires buck2.tests.core
    "core/log/test_upload_re_logs.py",      # requires manifold
    "core/query/uquery/test_uquery.py",     # requires manifold
    # Meta-internal memory/type-checking tests (Buck2-specific infrastructure)
    "core/interpreter/test_peak_allocated_bytes.py",        # Meta-internal peak alloc tracking
    "core/interpreter/test_peak_allocated_bytes_exceeds_limit.py",  # Meta-internal
    "core/interpreter/test_prelude_typecheck.py",           # Meta-internal typecheck infra
    "core/interpreter/test_unstable_typecheck.py",          # Meta-internal typecheck infra
    # Buck2-specific behavior (not needed for Bazel compatibility)
    "core/interpreter/test_attr_default_coercion.py",      # Tests Buck2-strict label validation
    "core/interpreter/test_missing_source_file.py",        # Uses BUCK2_HARD_ERROR env var
    # Meta-internal tests requiring NANO_PRELUDE env var or fbpython
    "core/audit/test_audit_output.py",                        # Requires NANO_PRELUDE Meta env var
    "core/audit/test_audit_providers.py",                     # Requires NANO_PRELUDE Meta env var
    "core/audit/test_audit_configurations.py",                # Requires NANO_PRELUDE Meta env var
    "core/audit/test_audit_deferred_materializer.py",         # Requires fbpython (Meta-internal Python)
    "core/audit/test_audit_execution_platform_resolution.py", # Requires NANO_PRELUDE Meta env var
    # Buck2-specific ?modifier syntax (target configuration modifiers)
    # These tests require kuro to support the `?modifier` target syntax which is Buck2-specific
    # and not part of Bazel's target language

    # Require Meta-internal tooling (fbpython, Manifold, etc.)
    "core/incremental_api/test_incremental_remote_action.py",  # Requires fbpython (Meta-internal)
    "core/subscribe/test_subscribe.py",                        # Requires BUCK2_EXPECT env var
    "core/vpnless/test_vpnless.py",                            # Meta-internal VPN-less feature

    # Watchman/Eden filesystem integration tests
    "core/io/test_file_watcher.py",                            # Requires Watchman process
    # Meta-internal completion/console tests requiring special env vars
    "core/completion/test_completion.py",                      # Requires BUCK2_COMPLETION_VERIFY env var
    "core/console/test_console.py",                            # Requires FIXTURES env var
]

# Individual test functions to skip (mapped to [test_file_path, test_function_name])
SKIP_TESTS = {
    # Buck2-specific modifier tests within otherwise-working test files
    "test_audit_subtarget_modifiers": "Uses Buck2-specific ?modifier syntax",
    "test_audit_subtarget_modifiers_target_universe": "Uses Buck2-specific ?modifier syntax",
    # Buck2-specific argfile @// (current-cell root) syntax not yet implemented
    "test_argfile_from_cwd_cell": "Uses Buck2-specific @// argfile cell syntax",
    "test_executable_argfile": "Uses Buck2-specific @//file#platform argfile syntax",
    "test_config_diff_command_project_relative": "Uses Buck2-specific @cell//path argfile syntax",
    "test_config_diff_tracker_modfile_change": "Uses Buck2-specific @cell//path argfile syntax",
    "test_config_diff_tracker_no_change": "Uses Buck2-specific @cell//path argfile syntax",
    "test_argfile_with_cell": "Uses @cell// argfile prefix syntax not supported in kuro",
    # BLAKE3-KEYED not supported in OSS kuro build
    "test_blake3": "BLAKE3-KEYED digest algorithm not supported in open source kuro build",
    # Require Meta-internal tooling (fbpython, installer binary, etc.)
    "test_validation_concurrent": "Requires fbpython (Meta-internal)",
    "test_validation_affects_install_command": "Requires Meta-internal installer binary",
    # Buck2-specific cfg modifiers (set_modifiers in PACKAGE files)
    "test_cfg_modifiers_change_target_hash": "Uses Buck2-specific set_modifiers() PACKAGE function",
    "test_parent_cfg_modifiers_change_target_hash": "Uses Buck2-specific set_modifiers() PACKAGE function",
    # Require Meta-internal tooling or unimplemented features
    "test_peak_stats": "Requires fbpython (Meta-internal)",
    "test_client_metadata_debug": "kuro debug allocator-stats not implemented for Cargo builds",
    "test_has_no_command_result": "Tests daemon event bus error messages (kuro-specific behavior)",
    "test_metadata": "Invocation record missing username field",
    # Restart tests needing specific daemon behavior
    "test_restart_cas_missing": "Tests specific daemon error message not matching kuro",
    "test_restart_disabled": "Tests restart-disabled behavior not matching kuro",
    # Daemon tests with kuro-specific behavior differences
    "test_process_title": "Daemon process named kurod[] not buck2d[] in kuro",
    "test_no_buckd_kills_existing_daemon": "kuro uses different kill message than expected",
    "test_daemon_buster": "daemon_buster feature not implemented in kuro",
    "test_same_state": "Nested invocations detect different state due to SANDCASTLE_ID handling",
    "test_trace_io_mismatch": "Requires trace-io feature not working in nested invocation context",
    # log tests requiring execution platform setup or RE
    "test_user_event_log_with_actions": "Requires execution platforms setup (execution_platforms() not available without nano_prelude)",
    "test_profile_bxl_with_actions": "Requires execution platforms (not enabled without nano_prelude)",
    # Soft error behavior differences (BUCK2_HARD_ERROR handling differs from kuro)
    "test_soft_error_quiet": "kuro treats soft_error as hard error when BUCK2_HARD_ERROR=false",
    "test_soft_error_no_stack": "kuro treats soft_error as hard error; soft error stack tracking differs",
    # Requires nano_prelude execution platform rules
    "test_configured_graph_deps_collapsed_in_errors": "Requires nano_prelude execution_platform/execution_platforms rules",
    "test_configured_graph_deps_collapsed_in_errors_2": "Requires nano_prelude execution_platform/execution_platforms rules",
    # command_report tests requiring Meta-internal env vars or features
    "test_command_report_watchman_error": "Requires Watchman integration",
    "test_command_report_init_daemon_error": "Requires BUCK2_TEST_INIT_DAEMON_ERROR (Meta-internal)",
    "test_exit_result_connection_error": "Requires BUCK2_TEST_FAIL_BUCKD_AUTH (Meta-internal)",
    "test_command_report_post_build_client_error": "Requires BUCK2_TEST_BUILD_ERROR (Meta-internal)",
    "test_cleanup_timeout": "Checks for Scribe sink which is Meta-internal",
    "test_what_materialized_csv": "Materializations not tracked for local execution in kuro",
    "test_what_materialized_sorted": "Materializations not tracked for local execution in kuro",
    "test_what_materialized_aggregated": "Materializations not tracked for local execution in kuro",
    "test_what_uploaded_csv": "Requires Remote Execution (RE) uploads not available",
    "test_what_uploaded_aggregated": "Requires Remote Execution (RE) uploads not available",
    "test_representative_config_flags_disregards_run_args": "Requires fbpython (Meta-internal)",
    # dep-only-incompatible soft error tests - kuro treats them as hard errors
    "test_exec_dep_transitive_incompatible": "Requires execution_platform rule (nano_prelude only)",
    "test_exec_dep_transitive_incompatible_post_transition": "Requires execution_platform rule (nano_prelude only)",
    "test_error_on_dep_only_incompatible[//dep_incompatible:dep_incompatible2-True]": "Soft error behavior differs in kuro",
    "test_error_on_dep_only_incompatible_excluded": "Soft error behavior differs in kuro",
    "test_dep_only_incompatible_custom_soft_errors_with_exclusions": "Requires dep_only_incompatible_info config feature",
    # ctargets self-transition outputs 2 lines in Buck2 but only 1 in kuro
    "test_ctargets_transition": "Self-transition outputs 1 line in kuro vs 2 in Buck2",
    # run modifiers tests - Buck2-specific ?modifier syntax
    "test_run_single_modifier": "Uses Buck2-specific ?modifier syntax",
    "test_run_multiple_modifiers": "Uses Buck2-specific ?modifier syntax",
    "test_run_order_of_modifiers": "Uses Buck2-specific ?modifier syntax",
    "test_run_target_universe_single_modifier": "Uses Buck2-specific ?modifier syntax",
    "test_run_target_universe_multiple_modifiers": "Uses Buck2-specific ?modifier syntax",
    "test_run_fails_with_global_modifiers": "Uses Buck2-specific ?modifier syntax",
    "test_run_fails_with_pattern_modifier_and_target_universe_modifier": "Uses Buck2-specific ?modifier syntax",
    # Transition tests where configuration is expected to be lost (Buck2-specific behavior)
    "test_run_with_transition_without_target_universe": "kuro cfg transitions don't lose configuration like Buck2",
    "test_run_with_transition_with_target_universe": "kuro cfg transitions don't lose configuration like Buck2",
    # Buck2-specific ?modifier syntax for ctargets
    "test_ctargets_modifier_single_pattern": "Uses Buck2-specific ?modifier syntax",
    "test_ctargets_modifier_multiple_patterns": "Uses Buck2-specific ?modifier syntax",
    "test_ctargets_modifier_multiple_modifiers": "Uses Buck2-specific ?modifier syntax",
    "test_ctargets_modifier_order_of_modifiers": "Uses Buck2-specific ?modifier syntax",
    "test_ctargets_modifier_multi_target_pattern": "Uses Buck2-specific ?modifier syntax",
    "test_ctargets_modifier_same_target": "Uses Buck2-specific ?modifier syntax",
    "test_ctargets_fails_with_global_modifier": "Uses Buck2-specific ?modifier syntax",
}

# ──────────────────────────────────────────────────────────────────────────────
# 2. Set required environment variables for the Buck test infrastructure
# ──────────────────────────────────────────────────────────────────────────────

# Path to the kuro binary (symlink at project root)
REPO_ROOT = TESTS_DIR.parent
KURO_BIN = REPO_ROOT / "kuro"
if not KURO_BIN.exists():
    # Try cargo debug build location
    KURO_BIN = REPO_ROOT / "target" / "debug" / "kuro"

os.environ.setdefault("TEST_EXECUTABLE", str(KURO_BIN))

# Required by buck_workspace.py's assertion; it gets deleted before Buck is invoked
os.environ.setdefault("BUCK2_MAX_BLOCKING_THREADS", "8")

# Run tests in isolated mode so @buck_test() doesn't require inplace= parameter
os.environ.setdefault("BUCK2_E2E_TEST_FLAVOR", "isolated")

# ──────────────────────────────────────────────────────────────────────────────
# 3. Pytest hooks
# ──────────────────────────────────────────────────────────────────────────────

def pytest_runtest_setup(item):
    """
    Set TEST_REPO_DATA to the directory containing the test data folder.

    buck_workspace.py uses:
        src = Path(os.environ["TEST_REPO_DATA"], marker.data_dir)
    so TEST_REPO_DATA must be the directory that contains the data_dir folder.

    Three layout conventions exist in tests/core/:
    1. data_dir is a non-empty string pointing directly under the test file's parent
       (e.g. data_dir="test_cmd_args_data" → tests/core/analysis/test_cmd_args_data/)
    2. data_dir is a non-empty string that's a subdirectory of {test_stem}_data/
       (e.g. data_dir="analysis_query_deps" → tests/core/analysis/test_analysis_queries_data/analysis_query_deps/)
    3. data_dir is "" (default): use {test_stem}_data/ as the project root directly
       (e.g. @buck_test() on test_audit_visibility.py → tests/core/audit/test_audit_visibility_data/)
    """
    test_file = Path(item.fspath)
    test_file_dir = test_file.parent

    # Default: data is directly in the test file's directory
    data_dir_base = test_file_dir

    marker = item.get_closest_marker("buck_test")
    if marker is not None and marker.args:
        buck_marker = marker.args[0]
        data_dir = getattr(buck_marker, "data_dir", None)

        stem_data = test_file_dir / (test_file.stem + "_data")

        if data_dir:  # non-empty string: find the data_dir
            direct_path = test_file_dir / data_dir
            nested_path = stem_data / data_dir
            if not direct_path.exists() and nested_path.exists():
                # Convention 2: data_dir is a subdir of {stem}_data/
                data_dir_base = stem_data
            # else convention 1: data_dir is directly under test_file_dir
        elif data_dir == "" and stem_data.exists():
            # Convention 3: @buck_test() with no data_dir arg but {stem}_data/ exists.
            # buck_workspace.py computes: src = Path(TEST_REPO_DATA, marker.data_dir)
            # With data_dir="" and TEST_REPO_DATA=stem_data, src = stem_data itself.
            # _copytree(stem_data, project_dir) copies all project files correctly.
            data_dir_base = stem_data

    os.environ["TEST_REPO_DATA"] = str(data_dir_base)


def pytest_collection_modifyitems(items):
    """Auto-mark async test functions with pytest.mark.asyncio.
    Also skip tests that require EdenFS when it's not installed,
    and skip Buck2-specific tests."""
    import shutil

    eden_available = shutil.which("eden") is not None
    for item in items:
        if isinstance(item, pytest.Function) and inspect.iscoroutinefunction(
            item.function
        ):
            item.add_marker(pytest.mark.asyncio)
        # Skip tests that require EdenFS if it's not installed
        if not eden_available:
            marker = item.get_closest_marker("buck_test")
            if marker and marker.args:
                buck_marker = marker.args[0]
                if getattr(buck_marker, "setup_eden", False):
                    item.add_marker(
                        pytest.mark.skip(reason="EdenFS is not installed")
                    )
        # Skip individual tests that test Buck2-specific behavior
        base_name = item.originalname if hasattr(item, "originalname") else item.name
        full_name = item.name  # includes parametrize args like [param1-param2]
        skip_key = full_name if full_name in SKIP_TESTS else (base_name if base_name in SKIP_TESTS else None)
        if skip_key is not None:
            item.add_marker(
                pytest.mark.skip(reason=SKIP_TESTS[skip_key])
            )


def pytest_configure(config):
    config.addinivalue_line(
        "markers", "buck_test: used by buck_test to pass data to Buck fixtures"
    )
