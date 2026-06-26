import json
import os
import re
import shutil
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SIBLING_NATIVELINK_BIN_CANDIDATES = [
    REPO_ROOT.parent / "nativelink" / "target" / "smol" / "nativelink",
    REPO_ROOT.parent / "nativelink" / "target" / "debug" / "nativelink",
]
SHELL_FIXTURE_ROOT = REPO_ROOT / "tests/core/executor/test_outputs_ordering_data"
CC_ACTIONS_FIXTURE_ROOT = REPO_ROOT / "tests/plan34/fixtures/cc_actions"
RULES_CC_FIXTURE_ROOT = REPO_ROOT / "tests/plan34/fixtures/rules_cc"
PLATFORM_EXEC_PROPERTIES_FIXTURE_ROOT = (
    REPO_ROOT / "tests/plan34/fixtures/platform_exec_properties"
)
DIRECT_LOCAL_EXECUTORS = {"Local", "LocalWorker", "Worker", "WorkerInit"}
PLAN34_EVIDENCE_ENV = "SLUG_PLAN34_EVIDENCE_JSONL"


def _is_executable(path: Path) -> bool:
    return path.is_file() and os.access(path, os.X_OK)


def _executable_path(path: Path) -> Path | None:
    if _is_executable(path):
        return path.resolve()
    if os.name == "nt" and path.suffix != ".exe":
        exe_path = path.with_suffix(".exe")
        if _is_executable(exe_path):
            return exe_path.resolve()
    return None


def _existing_executable_from_env(name: str) -> Path | None:
    value = os.environ.get(name)
    if not value:
        return None
    path = Path(value)
    executable = _executable_path(path)
    if executable is not None:
        return executable
    pytest.fail(f"{name}={value} is not an executable file")


def _slug_binary() -> Path:
    slug_bin = (
        _existing_executable_from_env("SLUG_BIN")
        or _existing_executable_from_env("TEST_EXECUTABLE")
        or REPO_ROOT / "target/debug/slug"
    )
    executable = _executable_path(slug_bin)
    if executable is not None:
        return executable
    pytest.skip("build target/debug/slug or set SLUG_BIN/TEST_EXECUTABLE")


def _nativelink_binary() -> Path:
    nativelink_bin = _existing_executable_from_env("SLUG_PLAN34_NATIVELINK_BIN")
    if nativelink_bin is not None:
        return nativelink_bin
    for candidate in SIBLING_NATIVELINK_BIN_CANDIDATES:
        if _is_executable(candidate):
            return candidate
    if (
        os.environ.get("GITHUB_ACTIONS") == "true"
        and os.environ.get("RUNNER_OS") == "Linux"
    ):
        pytest.fail(
            "Linux GitHub Actions must run .github/actions/setup_plan34_nativelink "
            "before tests/plan34/ so the REAPI smoke cannot pass by skipping"
        )
    pytest.skip(
        "set SLUG_PLAN34_NATIVELINK_BIN or build "
        "../nativelink/target/smol/nativelink or "
        "../nativelink/target/debug/nativelink to run the local REAPI executor smoke"
    )


def _write_executable(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    path.chmod(0o755)
    return path


def _append_plan34_evidence(record: dict) -> None:
    output = os.environ.get(PLAN34_EVIDENCE_ENV)
    if not output:
        return
    path = Path(output)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as f:
        json.dump({"schema": 1, **record}, f, sort_keys=True)
        f.write("\n")


def _command_summary_line(build: subprocess.CompletedProcess[str]) -> str:
    for line in (build.stdout + build.stderr).splitlines():
        marker = "Commands: "
        if marker in line:
            return re.sub(r"\x1b\[[0-9;]*m", "", line[line.index(marker) :])
    return ""


def _record_reapi_execution_evidence(
    *,
    test_name: str,
    phase: str = "remote_execution",
    target: str,
    build: subprocess.CompletedProcess[str],
    materialized_outputs: list[Path],
    reapi_actions: list[dict],
    upload_records: list[dict],
) -> None:
    platform_properties: list[dict] = []
    for action in reapi_actions:
        props = action["reproducer"]["details"]["platform_properties"]
        if props not in platform_properties:
            platform_properties.append(props)

    _append_plan34_evidence(
        {
            "test": test_name,
            "phase": phase,
            "target": target,
            "remote_service": "local_nativelink",
            "executor_boundary": "reapi",
            "direct_local_actions": 0,
            "reapi_actions": len(reapi_actions),
            "cache_query_actions": 0,
            "cache_hit_actions": 0,
            "materialized_outputs": len(materialized_outputs),
            "upload_records": len(upload_records),
            "uploaded_digests": sum(
                record["digests_uploaded"] for record in upload_records
            ),
            "uploaded_bytes": sum(record["bytes_uploaded"] for record in upload_records),
            "platform_properties": platform_properties,
            "command_summary": _command_summary_line(build),
        }
    )


def _record_remote_action_cache_evidence(
    *,
    test_name: str,
    target: str,
    build: subprocess.CompletedProcess[str],
    materialized_outputs: list[Path],
    cache_hits: list[dict],
) -> None:
    _append_plan34_evidence(
        {
            "test": test_name,
            "phase": "remote_action_cache_hit",
            "target": target,
            "remote_service": "local_nativelink",
            "direct_local_actions": 0,
            "reapi_actions": 0,
            "cache_query_actions": len(cache_hits),
            "cache_hit_actions": len(cache_hits),
            "materialized_outputs": len(materialized_outputs),
            "upload_records": 0,
            "uploaded_digests": 0,
            "uploaded_bytes": 0,
            "command_summary": _command_summary_line(build),
        }
    )


def _assert_and_record_reapi_execution(
    slug_bin: Path,
    workspace: Path,
    isolation: str,
    *,
    test_name: str,
    target: str,
    build: subprocess.CompletedProcess[str],
    expected_count: int,
    phase: str = "remote_execution",
    action_key_fragments: list[str] | None = None,
) -> None:
    materialized_outputs = _assert_materialized_show_outputs(
        build,
        workspace,
        expected_count=1,
    )
    reapi_actions = _assert_reapi_what_ran(
        slug_bin,
        workspace,
        isolation,
        expected_count=expected_count,
        action_key_fragments=action_key_fragments,
    )
    upload_records = _assert_reapi_uploads(
        slug_bin,
        workspace,
        isolation,
        expected_count=expected_count,
    )
    _record_reapi_execution_evidence(
        test_name=test_name,
        phase=phase,
        target=target,
        build=build,
        materialized_outputs=materialized_outputs,
        reapi_actions=reapi_actions,
        upload_records=upload_records,
    )


def _assert_and_record_remote_action_cache_hit(
    slug_bin: Path,
    workspace: Path,
    isolation: str,
    *,
    test_name: str,
    target: str,
    build: subprocess.CompletedProcess[str],
    expected_count: int,
) -> None:
    materialized_outputs = _assert_materialized_show_outputs(
        build,
        workspace,
        expected_count=1,
    )
    cache_hits = _assert_remote_action_cache_hit(
        slug_bin,
        workspace,
        isolation,
        expected_count=expected_count,
    )
    _record_remote_action_cache_evidence(
        test_name=test_name,
        target=target,
        build=build,
        materialized_outputs=materialized_outputs,
        cache_hits=cache_hits,
    )


def test_plan34_evidence_writer_is_opt_in(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence" / "plan34.jsonl"
    monkeypatch.delenv(PLAN34_EVIDENCE_ENV, raising=False)

    _append_plan34_evidence({"test": "not-written"})

    assert not evidence.exists()

    monkeypatch.setenv(PLAN34_EVIDENCE_ENV, str(evidence))
    _append_plan34_evidence({"test": "written", "direct_local_actions": 0})

    records = [json.loads(line) for line in evidence.read_text().splitlines()]
    assert records == [
        {
            "schema": 1,
            "test": "written",
            "direct_local_actions": 0,
        }
    ]


def test_nativelink_binary_env_var_wins(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    env_bin = _write_executable(tmp_path / "env" / "nativelink")
    smol_bin = _write_executable(tmp_path / "smol" / "nativelink")

    monkeypatch.setenv("SLUG_PLAN34_NATIVELINK_BIN", str(env_bin))
    monkeypatch.setattr(
        sys.modules[__name__],
        "SIBLING_NATIVELINK_BIN_CANDIDATES",
        [smol_bin],
    )

    assert _nativelink_binary() == env_bin


def test_nativelink_binary_discovers_smol_before_debug(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    smol_bin = _write_executable(tmp_path / "smol" / "nativelink")
    debug_bin = _write_executable(tmp_path / "debug" / "nativelink")

    monkeypatch.delenv("SLUG_PLAN34_NATIVELINK_BIN", raising=False)
    monkeypatch.setattr(
        sys.modules[__name__],
        "SIBLING_NATIVELINK_BIN_CANDIDATES",
        [smol_bin, debug_bin],
    )

    assert _nativelink_binary() == smol_bin


def test_nativelink_binary_fails_on_linux_github_actions_without_binary(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SLUG_PLAN34_NATIVELINK_BIN", raising=False)
    monkeypatch.setenv("GITHUB_ACTIONS", "true")
    monkeypatch.setenv("RUNNER_OS", "Linux")
    monkeypatch.setattr(
        sys.modules[__name__],
        "SIBLING_NATIVELINK_BIN_CANDIDATES",
        [],
    )

    with pytest.raises(pytest.fail.Exception, match="setup_plan34_nativelink"):
        _nativelink_binary()


def test_nativelink_binary_skips_without_binary_outside_linux_github_actions(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SLUG_PLAN34_NATIVELINK_BIN", raising=False)
    monkeypatch.setenv("GITHUB_ACTIONS", "true")
    monkeypatch.setenv("RUNNER_OS", "macOS")
    monkeypatch.setattr(
        sys.modules[__name__],
        "SIBLING_NATIVELINK_BIN_CANDIDATES",
        [],
    )

    with pytest.raises(pytest.skip.Exception):
        _nativelink_binary()


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _port_ready(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.1)
        return sock.connect_ex(("127.0.0.1", port)) == 0


def _run(args: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and result.returncode != 0:
        pytest.fail(
            "command failed with exit code {}: {}\nstdout:\n{}\nstderr:\n{}".format(
                result.returncode,
                " ".join(args),
                result.stdout,
                result.stderr,
            )
        )
    return result


def _assert_materialized_show_outputs(
    build: subprocess.CompletedProcess[str],
    workspace: Path,
    expected_count: int,
) -> list[Path]:
    paths: list[Path] = []
    for line in (build.stdout + build.stderr).splitlines():
        line = line.strip()
        if not line or line.startswith("["):
            continue
        parts = line.split(maxsplit=1)
        if (
            len(parts) == 2
            and "//" in parts[0]
            and parts[1].startswith("buck-out/")
        ):
            path = workspace / parts[1]
            assert path.exists(), path
            if path.is_file():
                assert path.stat().st_size > 0, path
            paths.append(path)

    assert len(paths) == expected_count
    return paths


def _read_what_ran(
    slug_bin: Path,
    workspace: Path,
    isolation: str,
    *,
    emit_cache_queries: bool = False,
) -> list[dict]:
    args = [
        str(slug_bin),
        "--isolation-dir",
        isolation,
        "log",
        "what-ran",
        "--format",
        "json",
    ]
    if emit_cache_queries:
        args.append("--emit-cache-queries")

    what_ran = _run(
        args,
        cwd=workspace,
    )
    return [
        json.loads(line)
        for line in (what_ran.stdout + what_ran.stderr).splitlines()
        if line.startswith("{")
    ]


def _read_what_uploaded(slug_bin: Path, workspace: Path, isolation: str) -> list[dict]:
    what_uploaded = _run(
        [
            str(slug_bin),
            "--isolation-dir",
            isolation,
            "log",
            "what-uploaded",
            "--format",
            "json",
        ],
        cwd=workspace,
    )
    return [
        json.loads(line)
        for line in (what_uploaded.stdout + what_uploaded.stderr).splitlines()
        if line.startswith("{")
    ]


def _assert_reapi_what_ran(
    slug_bin: Path,
    workspace: Path,
    isolation: str,
    expected_count: int,
    action_key_fragments: list[str] | None = None,
) -> list[dict]:
    entries = _read_what_ran(slug_bin, workspace, isolation)
    assert entries
    direct_local_actions = [
        entry
        for entry in entries
        if entry["reproducer"]["executor"] in DIRECT_LOCAL_EXECUTORS
    ]
    assert direct_local_actions == []

    reapi_actions = [
        entry for entry in entries if entry["reproducer"]["executor"] == "Re"
    ]
    assert len(reapi_actions) == expected_count
    assert reapi_actions == entries
    for action in reapi_actions:
        details = action["reproducer"]["details"]
        assert details["executor_boundary"] == "reapi"
        assert details["platform_properties"] == {"cpu_count": "1"}
        assert details["digest"]

    if action_key_fragments is not None:
        action_keys = {
            entry["reproducer"]["details"].get("action_key", "")
            for entry in reapi_actions
        }
        for fragment in action_key_fragments:
            assert any(fragment in action_key for action_key in action_keys)

    return reapi_actions


def _assert_remote_action_cache_hit(
    slug_bin: Path,
    workspace: Path,
    isolation: str,
    expected_count: int,
) -> list[dict]:
    entries = _read_what_ran(
        slug_bin,
        workspace,
        isolation,
        emit_cache_queries=True,
    )
    assert entries
    assert [
        entry
        for entry in entries
        if entry["reproducer"]["executor"] in DIRECT_LOCAL_EXECUTORS
    ] == []
    assert [
        entry for entry in entries if entry["reproducer"]["executor"] == "Re"
    ] == []

    cache_queries = [
        entry for entry in entries if entry["reproducer"]["executor"] == "CacheQuery"
    ]
    cache_hits = [entry for entry in entries if entry["reproducer"]["executor"] == "Cache"]
    assert len(cache_queries) == expected_count
    assert len(cache_hits) == expected_count
    assert len(entries) == expected_count * 2

    query_digests = {
        entry["reproducer"]["details"]["digest"] for entry in cache_queries
    }
    hit_digests = {entry["reproducer"]["details"]["digest"] for entry in cache_hits}
    assert query_digests == hit_digests
    assert all(entry["reproducer"]["details"]["digest"] for entry in entries)
    return cache_hits


def _assert_reapi_uploads(
    slug_bin: Path,
    workspace: Path,
    isolation: str,
    expected_count: int,
) -> list[dict]:
    records = _read_what_uploaded(slug_bin, workspace, isolation)
    assert len(records) == expected_count
    assert sum(record["digests_uploaded"] for record in records) > 0
    assert sum(record["bytes_uploaded"] for record in records) > 0
    assert all(record["action"] for record in records)
    return records


def _remove_local_action_cache_state(workspace: Path, isolation: str) -> None:
    shutil.rmtree(
        workspace / "buck-out" / isolation / "cache" / "action_cache_state",
        ignore_errors=True,
    )


def _write_nativelink_config(path: Path, root: Path, frontend_port: int, worker_port: int) -> None:
    path.write_text(
        f"""
{{
  stores: [
    {{
      name: "CAS",
      fast_slow: {{
        fast: {{
          filesystem: {{
            content_path: "{root}/cas/fast-content",
            temp_path: "{root}/cas/fast-temp",
            eviction_policy: {{ max_bytes: 1000000000 }},
          }},
        }},
        slow: {{
          filesystem: {{
            content_path: "{root}/cas/slow-content",
            temp_path: "{root}/cas/slow-temp",
            eviction_policy: {{ max_bytes: 1000000000 }},
          }},
        }},
      }},
    }},
    {{
      name: "AC",
      filesystem: {{
        content_path: "{root}/ac/content",
        temp_path: "{root}/ac/temp",
        eviction_policy: {{ max_bytes: 1000000000 }},
      }},
    }},
  ],
  schedulers: [
    {{
      name: "SIMPLE",
      simple: {{
        supported_platform_properties: {{
          cpu_count: "minimum",
        }},
      }},
    }},
  ],
  workers: [
    {{
      local: {{
        name: "worker-1",
        worker_api_endpoint: {{ uri: "grpc://127.0.0.1:{worker_port}" }},
        cas_fast_slow_store: "CAS",
        upload_action_result: {{ ac_store: "AC" }},
        work_directory: "{root}/worker/work",
        platform_properties: {{
          cpu_count: {{ values: ["1"] }},
        }},
      }},
    }},
  ],
  servers: [
    {{
      listener: {{ http: {{ socket_address: "127.0.0.1:{frontend_port}" }} }},
      services: {{
        cas: [{{ cas_store: "CAS" }}],
        ac: [{{ ac_store: "AC" }}],
        execution: [{{ cas_store: "CAS", scheduler: "SIMPLE" }}],
        bytestream: [
          {{ instance_name: "", cas_store: "CAS" }},
        ],
        capabilities: [{{ remote_execution: {{ scheduler: "SIMPLE" }} }}],
      }},
    }},
    {{
      listener: {{ http: {{ socket_address: "127.0.0.1:{worker_port}" }} }},
      services: {{ worker_api: {{ scheduler: "SIMPLE" }} }},
    }},
  ],
}}
""".lstrip(),
        encoding="utf-8",
    )


def _copy_fixture(source: Path, workspace: Path, files: list[str]) -> None:
    for name in files:
        shutil.copy2(source / name, workspace / name)


def _start_nativelink(
    nativelink_bin: Path,
    config: Path,
    frontend_port: int,
) -> tuple[subprocess.Popen[str], list[str]]:
    proc = subprocess.Popen(
        [str(nativelink_bin), str(config)],
        cwd=config.parent,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    lines: list[str] = []

    def collect_output() -> None:
        assert proc.stdout is not None
        for line in proc.stdout:
            lines.append(line)

    thread = threading.Thread(target=collect_output, daemon=True)
    thread.start()

    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            pytest.fail("NativeLink exited before the smoke:\n" + "".join(lines))
        worker_ready = any("Worker registered with scheduler" in line for line in lines)
        if _port_ready(frontend_port) and worker_ready:
            return proc, lines
        time.sleep(0.05)

    proc.terminate()
    pytest.fail("NativeLink did not become ready:\n" + "".join(lines))


def test_native_link_re_config_default_uses_reapi_without_remote_only(
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
    isolation = "plan34-reapi-smoke"
    remote_endpoint = f"grpc://127.0.0.1:{frontend_port}"

    try:
        build = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "build",
                "//:foo",
                "--show-output",
                f"--remote_executor={remote_endpoint}",
                f"--remote_cache={remote_endpoint}",
                "--remote_default_exec_properties=cpu_count=1",
            ],
            cwd=workspace,
        )
        build_output = build.stdout + build.stderr
        assert "BUILD SUCCEEDED" in build_output
        assert "Commands: 1 (cached: 0, remote: 1, local: 0)" in build_output
        assert "RE Session:" in build_output
        _assert_and_record_reapi_execution(
            slug_bin,
            workspace,
            isolation,
            test_name="test_native_link_re_config_default_uses_reapi_without_remote_only",
            target="//:foo",
            build=build,
            expected_count=1,
        )
    finally:
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace, check=False)
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(nativelink_lines))


def test_native_link_bare_remote_executor_supplies_reapi_cache_endpoint(
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
    isolation = "plan34-bare-remote-executor-smoke"
    remote_endpoint = f"grpc://127.0.0.1:{frontend_port}"

    try:
        build = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "build",
                "//:foo",
                "--show-output",
                f"--remote_executor={remote_endpoint}",
                "--remote_default_exec_properties=cpu_count=1",
            ],
            cwd=workspace,
        )
        build_output = build.stdout + build.stderr
        assert "BUILD SUCCEEDED" in build_output
        assert "Commands: 1 (cached: 0, remote: 1, local: 0)" in build_output
        assert "RE Session:" in build_output
        _assert_and_record_reapi_execution(
            slug_bin,
            workspace,
            isolation,
            test_name="test_native_link_bare_remote_executor_supplies_reapi_cache_endpoint",
            target="//:foo",
            build=build,
            expected_count=1,
        )
    finally:
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace, check=False)
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(nativelink_lines))


def test_native_link_remote_action_cache_hit_uses_reapi_without_local_fallback(
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
    isolation = "plan34-reapi-remote-ac-hit-smoke"
    remote_endpoint = f"grpc://127.0.0.1:{frontend_port}"

    try:
        first = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "build",
                "//:foo",
                "--show-output",
                f"--remote_executor={remote_endpoint}",
                f"--remote_cache={remote_endpoint}",
                "--remote_default_exec_properties=cpu_count=1",
                "--remote-only",
            ],
            cwd=workspace,
        )
        first_output = first.stdout + first.stderr
        assert "BUILD SUCCEEDED" in first_output
        assert "Commands: 1 (cached: 0, remote: 1, local: 0)" in first_output
        assert "RE Session:" in first_output
        _assert_and_record_reapi_execution(
            slug_bin,
            workspace,
            isolation,
            test_name="test_native_link_remote_action_cache_hit_uses_reapi_without_local_fallback",
            phase="remote_execution_seed",
            target="//:foo",
            build=first,
            expected_count=1,
        )

        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace)
        _remove_local_action_cache_state(workspace, isolation)

        second = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "build",
                "//:foo",
                "--show-output",
                f"--remote_executor={remote_endpoint}",
                f"--remote_cache={remote_endpoint}",
                "--remote_default_exec_properties=cpu_count=1",
                "--remote-only",
            ],
            cwd=workspace,
        )
        second_output = second.stdout + second.stderr
        assert "BUILD SUCCEEDED" in second_output
        assert "Commands: 1 (cached: 1, remote: 0, local: 0)" in second_output
        assert "RE Session:" in second_output
        _assert_and_record_remote_action_cache_hit(
            slug_bin,
            workspace,
            isolation,
            test_name="test_native_link_remote_action_cache_hit_uses_reapi_without_local_fallback",
            target="//:foo",
            build=second,
            expected_count=1,
        )
    finally:
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace, check=False)
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(nativelink_lines))


def test_native_link_platform_exec_properties_use_reapi_without_local_fallback(
    tmp_path: Path,
) -> None:
    slug_bin = _slug_binary()
    nativelink_bin = _nativelink_binary()

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    _copy_fixture(
        PLATFORM_EXEC_PROPERTIES_FIXTURE_ROOT,
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
    isolation = "plan34-reapi-platform-exec-properties-smoke"
    remote_endpoint = f"grpc://127.0.0.1:{frontend_port}"

    try:
        build = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "build",
                "//:foo",
                "--show-output",
                f"--remote_executor={remote_endpoint}",
                f"--remote_cache={remote_endpoint}",
                "--platforms=//:re_platform",
            ],
            cwd=workspace,
        )
        build_output = build.stdout + build.stderr
        assert "BUILD SUCCEEDED" in build_output
        assert "Commands: 1 (cached: 0, remote: 1, local: 0)" in build_output
        assert "RE Session:" in build_output
        _assert_and_record_reapi_execution(
            slug_bin,
            workspace,
            isolation,
            test_name="test_native_link_platform_exec_properties_use_reapi_without_local_fallback",
            target="//:foo",
            build=build,
            expected_count=1,
        )
    finally:
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace, check=False)
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(nativelink_lines))


def test_native_link_cc_actions_reapi_executor_smoke(tmp_path: Path) -> None:
    slug_bin = _slug_binary()
    nativelink_bin = _nativelink_binary()

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    _copy_fixture(
        CC_ACTIONS_FIXTURE_ROOT,
        workspace,
        [".buckroot", "MODULE.bazel", "BUILD.bazel", "defs.bzl", "hello.c", "main.c"],
    )

    nativelink_root = tmp_path / "nativelink"
    nativelink_root.mkdir()
    frontend_port = _free_port()
    worker_port = _free_port()
    config = nativelink_root / "nativelink.json5"
    _write_nativelink_config(config, nativelink_root, frontend_port, worker_port)

    proc, nativelink_lines = _start_nativelink(nativelink_bin, config, frontend_port)
    isolation = "plan34-reapi-cc-actions-smoke"
    remote_endpoint = f"grpc://127.0.0.1:{frontend_port}"

    try:
        build = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "build",
                "//:hello",
                "--show-output",
                f"--remote_executor={remote_endpoint}",
                f"--remote_cache={remote_endpoint}",
                "--remote_default_exec_properties=cpu_count=1",
                "--remote-only",
            ],
            cwd=workspace,
        )
        build_output = build.stdout + build.stderr
        assert "BUILD SUCCEEDED" in build_output
        assert "RE Session:" in build_output
        assert "Commands: 3 (cached: 0, remote: 3, local: 0)" in build_output
        assert "local: 0" in build_output
        _assert_and_record_reapi_execution(
            slug_bin,
            workspace,
            isolation,
            test_name="test_native_link_cc_actions_reapi_executor_smoke",
            target="//:hello",
            build=build,
            expected_count=3,
        )
    finally:
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace, check=False)
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(nativelink_lines))


def test_native_link_rules_cc_reapi_executor_smoke(tmp_path: Path) -> None:
    slug_bin = _slug_binary()
    nativelink_bin = _nativelink_binary()

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    _copy_fixture(
        RULES_CC_FIXTURE_ROOT,
        workspace,
        [".buckroot", "MODULE.bazel", "BUILD.bazel", "hello.c"],
    )

    nativelink_root = tmp_path / "nativelink"
    nativelink_root.mkdir()
    frontend_port = _free_port()
    worker_port = _free_port()
    config = nativelink_root / "nativelink.json5"
    _write_nativelink_config(config, nativelink_root, frontend_port, worker_port)

    proc, nativelink_lines = _start_nativelink(nativelink_bin, config, frontend_port)
    isolation = "plan34-reapi-rules-cc-smoke"
    remote_endpoint = f"grpc://127.0.0.1:{frontend_port}"

    try:
        build = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "build",
                "//:hello",
                "--show-output",
                f"--remote_executor={remote_endpoint}",
                f"--remote_cache={remote_endpoint}",
                "--remote_default_exec_properties=cpu_count=1",
                "--remote-only",
            ],
            cwd=workspace,
        )
        build_output = build.stdout + build.stderr
        assert "BUILD SUCCEEDED" in build_output
        assert "RE Session:" in build_output
        assert "Commands: 2 (cached: 0, remote: 2, local: 0)" in build_output
        assert "local: 0" in build_output
        _assert_and_record_reapi_execution(
            slug_bin,
            workspace,
            isolation,
            test_name="test_native_link_rules_cc_reapi_executor_smoke",
            target="//:hello",
            build=build,
            expected_count=2,
            action_key_fragments=[" c_compile ", " cpp_link "],
        )
    finally:
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace, check=False)
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(nativelink_lines))
