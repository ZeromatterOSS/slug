import json
import os
import shutil
import socket
import subprocess
import threading
import time
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SIBLING_NATIVELINK_BIN = (
    REPO_ROOT.parent / "nativelink" / "target" / "debug" / "nativelink"
)
SHELL_FIXTURE_ROOT = REPO_ROOT / "tests/core/executor/test_outputs_ordering_data"
CC_ACTIONS_FIXTURE_ROOT = REPO_ROOT / "tests/plan34/fixtures/cc_actions"
RULES_CC_FIXTURE_ROOT = REPO_ROOT / "tests/plan34/fixtures/rules_cc"
PLATFORM_EXEC_PROPERTIES_FIXTURE_ROOT = (
    REPO_ROOT / "tests/plan34/fixtures/platform_exec_properties"
)


def _is_executable(path: Path) -> bool:
    return path.is_file() and os.access(path, os.X_OK)


def _existing_executable_from_env(name: str) -> Path | None:
    value = os.environ.get(name)
    if not value:
        return None
    path = Path(value)
    if _is_executable(path):
        return path
    pytest.fail(f"{name}={value} is not an executable file")


def _slug_binary() -> Path:
    slug_bin = _existing_executable_from_env("SLUG_BIN") or REPO_ROOT / "target/debug/slug"
    if _is_executable(slug_bin):
        return slug_bin
    pytest.skip("build target/debug/slug or set SLUG_BIN")


def _nativelink_binary() -> Path:
    nativelink_bin = _existing_executable_from_env("SLUG_PLAN34_NATIVELINK_BIN")
    if nativelink_bin is not None:
        return nativelink_bin
    if _is_executable(SIBLING_NATIVELINK_BIN):
        return SIBLING_NATIVELINK_BIN
    pytest.skip(
        "set SLUG_PLAN34_NATIVELINK_BIN or build "
        "../nativelink/target/debug/nativelink to run the local REAPI executor smoke"
    )


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

        what_ran = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "log",
                "what-ran",
                "--format",
                "json",
            ],
            cwd=workspace,
        )
        what_ran_lines = [
            json.loads(line)
            for line in (what_ran.stdout + what_ran.stderr).splitlines()
            if line.startswith("{")
        ]
        reapi_actions = [
            entry for entry in what_ran_lines if entry["reproducer"]["executor"] == "Re"
        ]
        direct_local_actions = [
            entry
            for entry in what_ran_lines
            if entry["reproducer"]["executor"] in {"Local", "LocalWorker"}
        ]

        assert len(reapi_actions) == 1
        assert direct_local_actions == []
        action = reapi_actions[0]
        assert action["reproducer"]["details"]["platform_properties"] == {"cpu_count": "1"}
        assert action["reproducer"]["details"]["digest"]
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

        what_ran = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "log",
                "what-ran",
                "--format",
                "json",
            ],
            cwd=workspace,
        )
        what_ran_lines = [
            json.loads(line)
            for line in (what_ran.stdout + what_ran.stderr).splitlines()
            if line.startswith("{")
        ]
        reapi_actions = [
            entry for entry in what_ran_lines if entry["reproducer"]["executor"] == "Re"
        ]
        direct_local_actions = [
            entry
            for entry in what_ran_lines
            if entry["reproducer"]["executor"] in {"Local", "LocalWorker"}
        ]

        assert len(reapi_actions) == 1
        assert direct_local_actions == []
        action = reapi_actions[0]
        assert action["reproducer"]["details"]["platform_properties"] == {"cpu_count": "1"}
        assert action["reproducer"]["details"]["digest"]
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

        what_ran = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "log",
                "what-ran",
                "--format",
                "json",
            ],
            cwd=workspace,
        )
        what_ran_lines = [
            json.loads(line)
            for line in (what_ran.stdout + what_ran.stderr).splitlines()
            if line.startswith("{")
        ]
        reapi_actions = [
            entry for entry in what_ran_lines if entry["reproducer"]["executor"] == "Re"
        ]
        direct_local_actions = [
            entry
            for entry in what_ran_lines
            if entry["reproducer"]["executor"] in {"Local", "LocalWorker"}
        ]

        assert len(reapi_actions) == 3
        assert direct_local_actions == []
        for action in reapi_actions:
            assert action["reproducer"]["details"]["platform_properties"] == {
                "cpu_count": "1"
            }
            assert action["reproducer"]["details"]["digest"]
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
                f"--remote_executor={remote_endpoint}",
                f"--remote_cache={remote_endpoint}",
                "--remote_default_exec_properties=cpu_count=1",
                "--action_env=PATH=/usr/bin:/bin",
                "--remote-only",
            ],
            cwd=workspace,
        )
        build_output = build.stdout + build.stderr
        assert "BUILD SUCCEEDED" in build_output
        assert "RE Session:" in build_output
        assert "Commands: 2 (cached: 0, remote: 2, local: 0)" in build_output
        assert "local: 0" in build_output

        what_ran = _run(
            [
                str(slug_bin),
                "--isolation-dir",
                isolation,
                "log",
                "what-ran",
                "--format",
                "json",
            ],
            cwd=workspace,
        )
        what_ran_lines = [
            json.loads(line)
            for line in (what_ran.stdout + what_ran.stderr).splitlines()
            if line.startswith("{")
        ]
        reapi_actions = [
            entry for entry in what_ran_lines if entry["reproducer"]["executor"] == "Re"
        ]
        direct_local_actions = [
            entry
            for entry in what_ran_lines
            if entry["reproducer"]["executor"] in {"Local", "LocalWorker"}
        ]

        assert len(reapi_actions) == 2
        assert direct_local_actions == []
        action_keys = {
            entry["reproducer"]["details"]["action_key"] for entry in reapi_actions
        }
        assert any(" c_compile " in action_key for action_key in action_keys)
        assert any(" cpp_link " in action_key for action_key in action_keys)
        for action in reapi_actions:
            assert action["reproducer"]["details"]["platform_properties"] == {
                "cpu_count": "1"
            }
            assert action["reproducer"]["details"]["digest"]
    finally:
        _run([str(slug_bin), "--isolation-dir", isolation, "kill"], cwd=workspace, check=False)
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            pytest.fail("NativeLink did not terminate cleanly:\n" + "".join(nativelink_lines))
