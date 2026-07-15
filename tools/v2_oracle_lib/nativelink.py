"""NativeLink lifecycle management for the V2 oracle harness.

Starts a local NativeLink REAPI service (CAS/AC/Execution/worker) for fixtures
that require remote execution, and tears it down after the fixture run. The
config shape is ported from the archived V1 Plan 34 smoke harness at
``slug-v1-archive:tests/plan34/test_reapi_local_executor_smoke.py`` but rewritten
for the V2 oracle boundary: no Buck executor settings, no ``buck-out`` paths.
"""

from __future__ import annotations

import os
import socket
import subprocess
import threading
import time
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
READY_TIMEOUT_SECONDS = 20
READY_SIGNAL = "Worker registered with scheduler"


@dataclass(frozen=True)
class NativeLinkService:
    endpoint: str
    process: subprocess.Popen[str]
    config_path: Path
    root: Path
    log_lines: list[str]


def _is_executable(path: Path) -> bool:
    return path.is_file() and os.access(path, os.X_OK)


def discover_nativelink_binary() -> Path | None:
    env_path = os.environ.get("SLUG_V2_NATIVELINK_BIN")
    if env_path:
        path = Path(env_path)
        if _is_executable(path):
            return path
        raise FileNotFoundError(
            f"SLUG_V2_NATIVELINK_BIN={env_path} is not an executable file"
        )
    for candidate in (
        REPO_ROOT.parent / "nativelink" / "target" / "release" / "nativelink",
        REPO_ROOT.parent / "nativelink" / "target" / "smol" / "nativelink",
        REPO_ROOT.parent / "nativelink" / "target" / "debug" / "nativelink",
    ):
        if _is_executable(candidate):
            return candidate
    return None


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def _port_ready(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.25)
        return sock.connect_ex(("127.0.0.1", port)) == 0


def write_config(
    path: Path,
    root: Path,
    frontend_port: int,
    worker_port: int,
    worker_platform_properties: tuple[str, ...] = (),
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    # Build platform property entries. cpu_count is always present (minimum
    # match strategy); additional fixture-declared properties are added so the
    # scheduler can route actions that request them.
    scheduler_props_lines = ['          "cpu_count": "minimum",']
    worker_props_lines = ['          "cpu_count": { values: ["1"] },']
    for prop in worker_platform_properties:
        if "=" not in prop:
            raise ValueError(
                f"worker_platform_properties entry must be key=value, got {prop!r}"
            )
        key, val = prop.split("=", 1)
        # cpu_count uses the minimum (numeric) strategy; string properties use
        # exact match so the worker must provide the identical value.
        scheduler_props_lines.append(f'          "{key}": "exact",')
        worker_props_lines.append(f'          "{key}": {{ values: ["{val}"] }},')
    scheduler_props = "\n".join(scheduler_props_lines)
    worker_props = "\n".join(worker_props_lines)

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
{scheduler_props}
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
{worker_props}
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


def start_nativelink(
    binary: Path,
    root: Path,
    worker_platform_properties: tuple[str, ...] = (),
) -> NativeLinkService:
    root.mkdir(parents=True, exist_ok=True)
    frontend_port = _free_port()
    worker_port = _free_port()
    config_path = root / "nativelink.json5"
    write_config(config_path, root, frontend_port, worker_port, worker_platform_properties)
    process = subprocess.Popen(
        [str(binary), str(config_path)],
        cwd=config_path.parent,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    log_lines: list[str] = []

    def collect_output() -> None:
        assert process.stdout is not None
        for line in process.stdout:
            log_lines.append(line)

    thread = threading.Thread(target=collect_output, daemon=True)
    thread.start()

    deadline = time.monotonic() + READY_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(
                "NativeLink exited before becoming ready:\n" + "".join(log_lines)
            )
        worker_ready = any(READY_SIGNAL in line for line in log_lines)
        if _port_ready(frontend_port) and worker_ready:
            return NativeLinkService(
                endpoint=f"grpc://127.0.0.1:{frontend_port}",
                process=process,
                config_path=config_path,
                root=root,
                log_lines=log_lines,
            )
        time.sleep(0.05)

    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)
    raise RuntimeError("NativeLink did not become ready:\n" + "".join(log_lines))


def stop_nativelink(service: NativeLinkService) -> None:
    process = service.process
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
