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
from typing import BinaryIO

REPO_ROOT = Path(__file__).resolve().parents[2]
READY_TIMEOUT_SECONDS = 20
READY_SIGNAL = b"Worker registered with scheduler"
LOG_TAIL_BYTES = 16 * 1024
LOG_READ_BYTES = 4 * 1024
LOG_DRAIN_TIMEOUT_SECONDS = 1


class _NativeLinkLog:
    def __init__(self) -> None:
        self.worker_ready = threading.Event()
        self._lock = threading.Lock()
        self._tail = bytearray()

    def collect(self, stream: BinaryIO) -> None:
        overlap = b""
        with stream:
            while chunk := stream.read(LOG_READ_BYTES):
                with self._lock:
                    excess = len(self._tail) + len(chunk) - LOG_TAIL_BYTES
                    if excess > 0:
                        del self._tail[:excess]
                    self._tail.extend(chunk)
                if not self.worker_ready.is_set():
                    window = overlap + chunk
                    if READY_SIGNAL in window:
                        self.worker_ready.set()
                    else:
                        overlap = window[-(len(READY_SIGNAL) - 1):]

    def tail(self) -> bytes:
        with self._lock:
            return bytes(self._tail)


@dataclass(frozen=True)
class NativeLinkService:
    endpoint: str
    process: subprocess.Popen[bytes]
    config_path: Path
    root: Path
    _log: _NativeLinkLog
    _collector: threading.Thread

    def log_tail(self) -> bytes:
        """Snapshot the last LOG_TAIL_BYTES of merged process output."""
        return self._log.tail()


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

    # Client-facing CAS writes must validate bytes before persistent publication.
    # The local worker requires the raw FastSlowStore; both names share storage.
    # AC keys are action digests, so content verification does not apply to AC.
    path.write_text(
        f"""
{{
  stores: [
    {{
      name: "CAS_RAW",
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
      name: "CAS",
      verify: {{
        backend: {{ ref_store: {{ name: "CAS_RAW" }} }},
        verify_size: true,
        verify_hash: true,
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
        cas_fast_slow_store: "CAS_RAW",
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
          {{
            instance_name: "",
            cas_store: "CAS",
            // Keep interrupted-upload cleanup checks small in the local harness.
            persist_stream_on_disconnect_timeout: 1,
          }},
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
        bufsize=0,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    assert process.stdout is not None
    log = _NativeLinkLog()
    collector = threading.Thread(target=log.collect, args=(process.stdout,), daemon=True)
    service = NativeLinkService(
        endpoint=f"grpc://127.0.0.1:{frontend_port}",
        process=process,
        config_path=config_path,
        root=root,
        _log=log,
        _collector=collector,
    )
    collector.start()

    deadline = time.monotonic() + READY_TIMEOUT_SECONDS
    failure = "NativeLink did not become ready"
    try:
        while time.monotonic() < deadline:
            if process.poll() is not None:
                failure = "NativeLink exited before becoming ready"
                break
            if _port_ready(frontend_port) and log.worker_ready.is_set():
                return service
            time.sleep(0.05)
    except BaseException:
        stop_nativelink(service)
        raise

    stop_nativelink(service)
    raise RuntimeError(f"{failure}:\n" + service.log_tail().decode("utf-8", errors="replace"))


def stop_nativelink(service: NativeLinkService) -> None:
    process = service.process
    try:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    finally:
        service._collector.join(LOG_DRAIN_TIMEOUT_SECONDS)
