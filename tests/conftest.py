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
    "core/explain/test_explain.py",          # requires manifold
    "core/io/test_edenfs.py",                # requires buck2.tests.core
    "core/io/test_edenfs_aba.py",            # requires buck2.tests.core
    "core/io/test_fs_hash_crawler.py",       # requires buck2.tests.core
    "core/io/test_notify.py",               # requires buck2.tests.core
    "core/io/test_watchman.py",             # requires buck2.tests.core
    "core/io/test_watchman_aba.py",         # requires buck2.tests.core
    "core/log/test_upload_re_logs.py",      # requires manifold
    "core/query/uquery/test_uquery.py",     # requires manifold
]

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
    Also skip tests that require EdenFS when it's not installed."""
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


def pytest_configure(config):
    config.addinivalue_line(
        "markers", "buck_test: used by buck_test to pass data to Buck fixtures"
    )
