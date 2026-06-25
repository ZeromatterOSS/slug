import json
import os
import subprocess
from pathlib import Path

import pytest

from tests.plan34.test_reapi_local_executor_smoke import (
    DIRECT_LOCAL_EXECUTORS,
    SHELL_FIXTURE_ROOT,
    SIBLING_NATIVELINK_BIN,
    _assert_reapi_what_ran,
    _copy_fixture,
    _executable_path,
    _free_port,
    _is_executable,
    _run,
    _slug_binary,
    _start_nativelink,
    _write_nativelink_config,
)


def _nativelink_binary() -> Path:
    value = os.environ.get("SLUG_PLAN31_NATIVELINK_BIN")
    if value:
        executable = _executable_path(Path(value))
        if executable is not None:
            return executable
        pytest.fail(f"SLUG_PLAN31_NATIVELINK_BIN={value} is not executable")

    value = os.environ.get("SLUG_PLAN34_NATIVELINK_BIN")
    if value:
        executable = _executable_path(Path(value))
        if executable is not None:
            return executable
        pytest.fail(f"SLUG_PLAN34_NATIVELINK_BIN={value} is not executable")

    if _is_executable(SIBLING_NATIVELINK_BIN):
        return SIBLING_NATIVELINK_BIN

    pytest.skip(
        "set SLUG_PLAN31_NATIVELINK_BIN or build "
        "../nativelink/target/debug/nativelink to run the persistent RE AC smoke"
    )


def _read_what_ran_with_cache_queries(
    slug_bin: Path, workspace: Path, isolation: str
) -> list[dict]:
    what_ran = _run(
        [
            str(slug_bin),
            "--isolation-dir",
            isolation,
            "log",
            "what-ran",
            "--format",
            "json",
            "--emit-cache-queries",
        ],
        cwd=workspace,
    )
    return [
        json.loads(line)
        for line in (what_ran.stdout + what_ran.stderr).splitlines()
        if line.startswith("{")
    ]


def _build_with_reapi(
    slug_bin: Path, workspace: Path, isolation: str, endpoint: str
) -> subprocess.CompletedProcess[str]:
    return _run(
        [
            str(slug_bin),
            "--isolation-dir",
            isolation,
            "build",
            "//:foo",
            f"--remote_executor={endpoint}",
            f"--remote_cache={endpoint}",
            "--remote_default_exec_properties=cpu_count=1",
            "--remote-only",
        ],
        cwd=workspace,
    )


def _stop_nativelink(proc: subprocess.Popen[str], lines: list[str]) -> None:
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)
        pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(lines))


def test_persistent_re_action_cache_short_circuits_remote_cache_query(
    tmp_path: Path,
) -> None:
    slug_bin = _slug_binary()
    nativelink_bin = _nativelink_binary()

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    _copy_fixture(
        SHELL_FIXTURE_ROOT,
        workspace,
        [".buckroot", "MODULE.bazel", "BUILD.bazel", "defs.bzl"],
    )

    nativelink_root = tmp_path / "nativelink"
    nativelink_root.mkdir()
    frontend_port = _free_port()
    worker_port = _free_port()
    config = nativelink_root / "nativelink.json5"
    _write_nativelink_config(config, nativelink_root, frontend_port, worker_port)

    proc, nativelink_lines = _start_nativelink(nativelink_bin, config, frontend_port)
    isolation = "plan31-persistent-re-action-cache"
    endpoint = f"grpc://127.0.0.1:{frontend_port}"
    action_cache_db = (
        workspace
        / "buck-out"
        / isolation
        / "cache"
        / "action_cache_state"
        / "db.sqlite"
    )

    try:
        first = _build_with_reapi(slug_bin, workspace, isolation, endpoint)
        first_output = first.stdout + first.stderr
        assert "BUILD SUCCEEDED" in first_output
        assert "Commands: 1 (cached: 0, remote: 1, local: 0)" in first_output
        _assert_reapi_what_ran(slug_bin, workspace, isolation, expected_count=1)
        assert action_cache_db.is_file()
        assert action_cache_db.stat().st_size > 0

        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace)

        second = _build_with_reapi(slug_bin, workspace, isolation, endpoint)
        second_output = second.stdout + second.stderr
        assert "BUILD SUCCEEDED" in second_output
        assert "Commands: 1 (cached: 1, remote: 0, local: 0)" in second_output

        entries = _read_what_ran_with_cache_queries(slug_bin, workspace, isolation)
        assert entries
        direct_local_actions = [
            entry
            for entry in entries
            if entry["reproducer"]["executor"] in DIRECT_LOCAL_EXECUTORS
        ]
        assert direct_local_actions == []
        assert [
            entry
            for entry in entries
            if entry["reproducer"]["executor"] == "CacheQuery"
        ] == []
        assert [
            entry for entry in entries if entry["reproducer"]["executor"] == "Re"
        ] == []

        cache_hits = [
            entry for entry in entries if entry["reproducer"]["executor"] == "Cache"
        ]
        assert len(cache_hits) == 1
        assert cache_hits == entries
        assert cache_hits[0]["reproducer"]["details"]["digest"]
    finally:
        _run(
            [str(slug_bin), "--isolation-dir", isolation, "kill"],
            cwd=workspace,
            check=False,
        )
        _stop_nativelink(proc, nativelink_lines)


def test_stale_persistent_re_action_cache_reexecutes_through_reapi(
    tmp_path: Path,
) -> None:
    slug_bin = _slug_binary()
    nativelink_bin = _nativelink_binary()

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    _copy_fixture(
        SHELL_FIXTURE_ROOT,
        workspace,
        [".buckroot", "MODULE.bazel", "BUILD.bazel", "defs.bzl"],
    )

    isolation = "plan31-stale-persistent-re-action-cache"
    proc: subprocess.Popen[str] | None = None
    nativelink_lines: list[str] = []

    def start_clean_nativelink(name: str) -> tuple[str, subprocess.Popen[str], list[str]]:
        nativelink_root = tmp_path / name
        nativelink_root.mkdir()
        frontend_port = _free_port()
        worker_port = _free_port()
        config = nativelink_root / "nativelink.json5"
        _write_nativelink_config(config, nativelink_root, frontend_port, worker_port)
        next_proc, next_lines = _start_nativelink(
            nativelink_bin, config, frontend_port
        )
        return f"grpc://127.0.0.1:{frontend_port}", next_proc, next_lines

    try:
        endpoint, proc, nativelink_lines = start_clean_nativelink("nativelink-seed")
        first = _build_with_reapi(slug_bin, workspace, isolation, endpoint)
        first_output = first.stdout + first.stderr
        assert "BUILD SUCCEEDED" in first_output
        assert "Commands: 1 (cached: 0, remote: 1, local: 0)" in first_output
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace)
        _stop_nativelink(proc, nativelink_lines)
        proc = None

        endpoint, proc, nativelink_lines = start_clean_nativelink("nativelink-empty")
        second = _build_with_reapi(slug_bin, workspace, isolation, endpoint)
        second_output = second.stdout + second.stderr
        assert "BUILD SUCCEEDED" in second_output
        assert "Commands: 1 (cached: 0, remote: 1, local: 0)" in second_output

        entries = _read_what_ran_with_cache_queries(slug_bin, workspace, isolation)
        assert entries
        assert [
            entry
            for entry in entries
            if entry["reproducer"]["executor"] in DIRECT_LOCAL_EXECUTORS
        ] == []
        assert [
            entry
            for entry in entries
            if entry["reproducer"]["executor"] == "CacheQuery"
        ] == []
        assert any(entry["reproducer"]["executor"] == "Re" for entry in entries)

        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace)
        third = _build_with_reapi(slug_bin, workspace, isolation, endpoint)
        third_output = third.stdout + third.stderr
        assert "BUILD SUCCEEDED" in third_output
        assert "Commands: 1 (cached: 1, remote: 0, local: 0)" in third_output
    finally:
        _run(
            [str(slug_bin), "--isolation-dir", isolation, "kill"],
            cwd=workspace,
            check=False,
        )
        if proc is not None:
            _stop_nativelink(proc, nativelink_lines)
