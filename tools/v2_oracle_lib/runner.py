from __future__ import annotations

import hashlib
import json
import os
import select
import shutil
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .fixture import Fixture, FixtureCommand, Mutation
from .manifest import collect_manifest_roots
from .nativelink import NativeLinkService, discover_nativelink_binary, start_nativelink, stop_nativelink
from .normalize import normalize_text, path_replacements

REPO_ROOT = Path(__file__).resolve().parents[2]
HTTP_REGISTRY_STARTUP_TIMEOUT_SECONDS = 5.0
WORKSPACE_URI_TOKEN = "{{workspace_uri}}"


@dataclass(frozen=True)
class ToolConfig:
    name: str
    executable: Path


@dataclass(frozen=True)
class RunOptions:
    run_root: Path
    timeout_seconds: int = 120


def default_run_root() -> Path:
    configured = os.environ.get("SLUG_V2_ORACLE_ROOT")
    if configured:
        return Path(configured)
    return REPO_ROOT / "target" / "v2o"


def _copy_workspace(fixture: Fixture, run_dir: Path) -> Path:
    workspace = run_dir / "workspace"
    if workspace.exists():
        raise FileExistsError(f"refusing to reuse run workspace: {workspace}")
    shutil.copytree(fixture.workspace, workspace, symlinks=True)
    _expand_workspace_uri_templates(workspace)
    return workspace


def _workspace_uri(workspace: Path) -> str:
    return workspace.resolve().as_uri()


def _expand_workspace_uri(value: str | None, workspace_uri: str) -> str | None:
    if value is None:
        return None
    return value.replace(WORKSPACE_URI_TOKEN, workspace_uri)


def _expand_workspace_uri_templates(workspace: Path) -> None:
    workspace_uri = _workspace_uri(workspace)
    for directory, dirnames, filenames in os.walk(workspace, followlinks=False):
        directory_path = Path(directory)
        dirnames[:] = [
            name for name in dirnames if not (directory_path / name).is_symlink()
        ]
        for name in filenames:
            path = directory_path / name
            if path.is_symlink() or not path.is_file():
                continue
            try:
                content = path.read_bytes().decode("utf-8")
            except UnicodeDecodeError:
                continue
            if WORKSPACE_URI_TOKEN in content:
                path.write_bytes(
                    content.replace(WORKSPACE_URI_TOKEN, workspace_uri).encode("utf-8")
                )


def _start_fixture_http_registry(
    fixture: Fixture, workspace: Path
) -> tuple[subprocess.Popen[bytes], str, Path]:
    service = fixture.root / "http_registry.py"
    registry = workspace / "registry"
    log = workspace / "http_registry_requests.jsonl"
    process = subprocess.Popen(
        [
            sys.executable,
            str(service),
            "--root",
            str(registry),
            "--log",
            str(log),
            "--port",
            str(fixture.http_registry_port or 0),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        assert process.stdout is not None
        stdout = b""
        deadline = time.monotonic() + HTTP_REGISTRY_STARTUP_TIMEOUT_SECONDS
        while b"\n" not in stdout:
            if process.poll() is not None:
                raise RuntimeError("service exited before publishing its endpoint")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError("timed out waiting for the service endpoint")
            readable, _, _ = select.select(
                [process.stdout], [], [], min(remaining, 0.1)
            )
            if readable:
                chunk = os.read(process.stdout.fileno(), 4096)
                if not chunk:
                    raise RuntimeError("service closed stdout before publishing its endpoint")
                stdout += chunk
        endpoint = stdout.splitlines()[0].decode("utf-8").strip()
        if not endpoint.startswith("http://127.0.0.1:"):
            raise RuntimeError(f"invalid service endpoint: {endpoint!r}")
    except Exception as error:
        _stop_process(process)
        assert process.stderr is not None
        stderr = process.stderr.read().decode("utf-8", errors="replace").strip()
        detail = f": {stderr}" if stderr else ""
        raise RuntimeError(f"fixture HTTP registry failed to start{detail}") from error

    try:
        for path in workspace.rglob("*"):
            if path.is_file() and path != log:
                try:
                    content = path.read_text(encoding="utf-8")
                except UnicodeDecodeError:
                    continue
                if "{{http_registry}}" in content:
                    path.write_text(
                        content.replace("{{http_registry}}", endpoint),
                        encoding="utf-8",
                        newline="",
                    )
    except Exception:
        _stop_process(process)
        raise
    return process, endpoint, log


def _stop_process(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        process.wait()
        return
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _collect_manifest(
    workspace: Path,
    roots: list[str] | tuple[str, ...],
    http_registry_endpoint: str | None,
) -> list[dict[str, Any]]:
    manifest = collect_manifest_roots(workspace, roots)
    if http_registry_endpoint is None or "MODULE.bazel.lock" not in roots:
        return manifest

    lockfile = workspace / "MODULE.bazel.lock"
    if not lockfile.is_file():
        return manifest
    normalized = lockfile.read_bytes().replace(
        http_registry_endpoint.encode("utf-8"), b"<http_registry>"
    )
    for entry in manifest:
        if (
            entry["root"] == "MODULE.bazel.lock"
            and entry["path"] == "MODULE.bazel.lock"
            and entry["type"] == "file"
        ):
            entry["digest"] = hashlib.sha256(normalized).hexdigest()
            entry["size"] = len(normalized)
            entry["http_registry_endpoint_normalized"] = True
    return manifest


def _apply_mutations(workspace: Path, command: FixtureCommand) -> list[dict[str, str | None]]:
    applied: list[dict[str, str | None]] = []
    workspace_uri = _workspace_uri(workspace)
    for mutation in command.mutations:
        path = (
            _workspace_mutation_entry_path(workspace, mutation.path)
            if mutation.op in {"delete", "rename"}
            else _workspace_mutation_path(workspace, mutation.path)
        )
        if mutation.op == "create":
            if path.exists() or path.is_symlink():
                raise FileExistsError(f"mutation create destination exists: {mutation.path}")
            _require_existing_real_parent(path, mutation.path)
            content = _expand_workspace_uri(mutation.content, workspace_uri) or ""
            path.write_text(content, encoding="utf-8", newline="")
            record: dict[str, str | None] = {"op": "create", "path": mutation.path}
            if mutation.content is not None and WORKSPACE_URI_TOKEN in mutation.content:
                record["content"] = mutation.content
            applied.append(record)
            continue
        if mutation.op == "delete":
            _require_regular_or_symlink_source(path, mutation.path)
            path.unlink()
            applied.append({"op": "delete", "path": mutation.path})
            continue
        if mutation.op == "rename":
            assert mutation.destination is not None
            destination = _workspace_mutation_entry_path(workspace, mutation.destination)
            _require_regular_or_symlink_source(path, mutation.path)
            if os.path.lexists(destination):
                raise FileExistsError(f"mutation rename destination exists: {mutation.destination}")
            _require_existing_real_parent(destination, mutation.destination)
            path.rename(destination)
            applied.append({"op": "rename", "path": mutation.path, "destination": mutation.destination})
            continue
        if not path.is_file():
            raise FileNotFoundError(f"mutation target does not exist: {mutation.path}")
        if mutation.content is not None:
            old = path.read_text(encoding="utf-8")
            content = _expand_workspace_uri(mutation.content, workspace_uri)
            assert content is not None
            path.write_text(content, encoding="utf-8", newline="")
            record = {
                "path": mutation.path,
                "find": None,
                "replace": None,
                "old_digest_hint": str(len(old)),
            }
            if WORKSPACE_URI_TOKEN in mutation.content:
                record["content"] = mutation.content
            applied.append(record)
            continue
        old = path.read_text(encoding="utf-8")
        assert mutation.find is not None
        assert mutation.replace is not None
        find = _expand_workspace_uri(mutation.find, workspace_uri)
        replace = _expand_workspace_uri(mutation.replace, workspace_uri)
        assert find is not None
        assert replace is not None
        if find not in old:
            raise ValueError(f"mutation text not found in {mutation.path}: {mutation.find!r}")
        path.write_text(old.replace(find, replace), encoding="utf-8", newline="")
        applied.append({"path": mutation.path, "find": mutation.find, "replace": mutation.replace, "old_digest_hint": str(len(old))})
    return applied


def _workspace_mutation_path(workspace: Path, path: str) -> Path:
    workspace_root = workspace.resolve()
    candidate = workspace / path
    resolved = candidate.resolve(strict=False)
    if not resolved.is_relative_to(workspace_root):
        raise ValueError(f"mutation path escapes workspace: {path}")
    return candidate


def _workspace_mutation_entry_path(workspace: Path, path: str) -> Path:
    workspace_root = workspace.resolve()
    candidate = workspace / path
    resolved_parent = candidate.parent.resolve(strict=False)
    if not resolved_parent.is_relative_to(workspace_root):
        raise ValueError(f"mutation path escapes workspace: {path}")
    return candidate


def _require_regular_or_symlink_source(path: Path, display_path: str) -> None:
    if not os.path.lexists(path):
        raise FileNotFoundError(f"mutation source does not exist: {display_path}")
    mode = path.lstat().st_mode
    if not (stat.S_ISREG(mode) or stat.S_ISLNK(mode)):
        raise ValueError(
            f"mutation source must be a regular file or symlink: {display_path}"
        )


def _require_existing_real_parent(path: Path, display_path: str) -> None:
    parent = path.parent
    if not parent.is_dir() or parent.is_symlink():
        raise FileNotFoundError(
            f"mutation destination parent must be an existing real directory: {display_path}"
        )


def _argv(tool: ToolConfig, command: FixtureCommand, output_base: Path, daemon: bool = False) -> list[str]:
    if tool.name == "bazel":
        return [str(tool.executable), f"--output_base={output_base}", *command.argv]
    if daemon:
        return [str(tool.executable), f"--output_base={output_base}", *command.argv]
    return [str(tool.executable), *command.argv]


def _shutdown_slug_daemon(output_base: Path) -> None:
    """Send a shutdown command to the slug daemon if its socket exists."""
    socket_path = output_base / "slugd.sock"
    if not socket_path.exists():
        return
    try:
        import socket as _socket

        sock = _socket.socket(_socket.AF_UNIX, _socket.SOCK_STREAM)
        sock.settimeout(2.0)
        sock.connect(str(socket_path))
        sock.sendall(b"shutdown\n")
        sock.close()
    except OSError:
        pass
    # Clean up stale socket/pid files.
    for name in ("slugd.sock", "slugd.pid"):
        stale = output_base / name
        if stale.exists():
            try:
                stale.unlink()
            except OSError:
                pass


def _slug_reapi_argv(
    argv: list[str],
    command: FixtureCommand,
    endpoint: str,
    default_exec_properties: tuple[str, ...],
) -> list[str]:
    result = list(argv)
    if command.argv[0] != "build":
        return result
    result.append(f"--remote_executor={endpoint}")
    for prop in default_exec_properties:
        result.append(f"--remote_default_exec_properties={prop}")
    return result


def _extract_reapi_evidence(stderr: str) -> dict[str, Any] | None:
    # The slug build command emits one JSON object on stderr. The captured
    # stderr string contains that object with quotes intact, so a direct
    # json.loads on the last non-empty line is the robust extraction.
    for line in reversed(stderr.splitlines()):
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if parsed.get("completed_boundary") == "reapi_native_execution":
            return parsed
    return None


def run_fixture(fixture: Fixture, tool: ToolConfig, options: RunOptions) -> dict[str, Any]:
    run_id = time.strftime("%Y%m%d-%H%M%S") + f"-{os.getpid()}-{tool.name}"
    run_dir = options.run_root / "runs" / fixture.name / run_id
    run_dir.mkdir(parents=True, exist_ok=False)
    workspace = _copy_workspace(fixture, run_dir)
    output_base = options.run_root / "ob" / fixture.name / tool.name
    output_base.mkdir(parents=True, exist_ok=True)
    replacements = path_replacements(workspace=workspace, run_dir=run_dir, output_base=output_base)

    nativelink_service: NativeLinkService | None = None
    registry_service: subprocess.Popen[bytes] | None = None
    registry_log: Path | None = None
    registry_endpoint: str | None = None
    if fixture.reapi.remote_executor and tool.name == "slug":
        binary = discover_nativelink_binary()
        if binary is None:
            raise FileNotFoundError(
                "fixture requires NativeLink but no binary was found; set "
                "SLUG_V2_NATIVELINK_BIN or build ../nativelink/target/release/nativelink"
            )
        nativelink_service = start_nativelink(
            binary,
            run_dir / "nativelink",
            fixture.reapi.worker_platform_properties,
        )

    try:
        if fixture.http_registry:
            registry_service, registry_endpoint, registry_log = _start_fixture_http_registry(
                fixture, workspace
            )
            replacements.update(path_replacements(http_registry=registry_endpoint))
        records: list[dict[str, Any]] = []
        for command in fixture.commands:
            mutations = _apply_mutations(workspace, command)
            argv = _argv(tool, command, output_base, fixture.daemon)
            if registry_endpoint is not None:
                argv = [
                    argument.replace("{{http_registry}}", registry_endpoint)
                    for argument in argv
                ]
            if nativelink_service is not None:
                argv = _slug_reapi_argv(
                    argv,
                    command,
                    nativelink_service.endpoint,
                    fixture.reapi.default_exec_properties,
                )
            env = os.environ.copy()
            env_overrides = dict(command.env)
            env.update(env_overrides)
            start = time.monotonic()
            completed = subprocess.run(
                argv,
                cwd=workspace,
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=options.timeout_seconds,
                check=False,
            )
            duration_ms = int((time.monotonic() - start) * 1000)
            manifest_roots = command.manifest_roots or fixture.manifest_roots
            registry_request_counts: dict[str, int] | None = None
            if registry_log is not None:
                requests = (
                    [
                        json.loads(line)
                        for line in registry_log.read_text(encoding="utf-8").splitlines()
                    ]
                    if registry_log.exists()
                    else []
                )
                registry_request_counts = {
                    path: sum(request["path"] == path for request in requests)
                    for path in sorted({request["path"] for request in requests})
                }
                (workspace / "http_registry_request_counts.json").write_text(
                    json.dumps(registry_request_counts, sort_keys=True) + "\n",
                    encoding="utf-8",
                )
            record: dict[str, Any] = {
                "name": command.name,
                "argv": command.argv,
                "executed_argv": argv,
                "env_allowlist": {key: env.get(key) for key in command.env_allowlist},
                "env_overrides": env_overrides,
                "cwd": str(workspace),
                "exit_code": completed.returncode,
                "stdout": completed.stdout,
                "stderr": completed.stderr,
                "normalized_stdout": normalize_text(completed.stdout, replacements),
                "normalized_stderr": normalize_text(completed.stderr, replacements),
                "duration_ms": duration_ms,
                "mutations": mutations,
                "manifest": _collect_manifest(
                    workspace, manifest_roots, registry_endpoint
                ),
            }
            if nativelink_service is not None:
                record["reapi_evidence"] = _extract_reapi_evidence(completed.stderr)
                record["reapi_endpoint"] = nativelink_service.endpoint
            if registry_request_counts is not None:
                record["http_registry_request_counts"] = registry_request_counts
            records.append(record)
    finally:
        if fixture.daemon and tool.name == "slug":
            _shutdown_slug_daemon(output_base)
        if nativelink_service is not None:
            stop_nativelink(nativelink_service)
        if registry_service is not None:
            _stop_process(registry_service)

    result = {
        "schema_version": 1,
        "fixture": fixture.name,
        "tool": tool.name,
        "tool_executable": str(tool.executable),
        "run_dir": str(run_dir),
        "workspace": str(workspace),
        "output_base": str(output_base),
        "commands": records,
    }
    (run_dir / f"{tool.name}.json").write_text(json.dumps(result, indent=2, sort_keys=True), encoding="utf-8")
    return result
