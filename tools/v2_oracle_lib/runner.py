from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .fixture import Fixture, FixtureCommand
from .manifest import collect_manifest_roots
from .nativelink import NativeLinkService, discover_nativelink_binary, start_nativelink, stop_nativelink
from .normalize import normalize_text, path_replacements

REPO_ROOT = Path(__file__).resolve().parents[2]


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
    return workspace


def _apply_mutations(workspace: Path, command: FixtureCommand) -> list[dict[str, str | None]]:
    applied: list[dict[str, str | None]] = []
    for mutation in command.mutations:
        path = workspace / mutation.path
        if not path.is_file():
            raise FileNotFoundError(f"mutation target does not exist: {mutation.path}")
        if mutation.content is not None:
            old = path.read_text(encoding="utf-8")
            path.write_text(mutation.content, encoding="utf-8", newline="")
            applied.append({"path": mutation.path, "find": None, "replace": None, "old_digest_hint": str(len(old))})
            continue
        old = path.read_text(encoding="utf-8")
        assert mutation.find is not None
        assert mutation.replace is not None
        if mutation.find not in old:
            raise ValueError(f"mutation text not found in {mutation.path}: {mutation.find!r}")
        path.write_text(old.replace(mutation.find, mutation.replace), encoding="utf-8", newline="")
        applied.append({"path": mutation.path, "find": mutation.find, "replace": mutation.replace, "old_digest_hint": str(len(old))})
    return applied


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
    service: NativeLinkService,
    default_exec_properties: tuple[str, ...],
) -> list[str]:
    result = list(argv)
    result.append(f"--remote_executor={service.endpoint}")
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
        records: list[dict[str, Any]] = []
        for command in fixture.commands:
            mutations = _apply_mutations(workspace, command)
            argv = _argv(tool, command, output_base, fixture.daemon)
            if nativelink_service is not None:
                argv = _slug_reapi_argv(
                    argv,
                    nativelink_service,
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
                "manifest": collect_manifest_roots(workspace, manifest_roots),
            }
            if nativelink_service is not None:
                record["reapi_evidence"] = _extract_reapi_evidence(completed.stderr)
                record["reapi_endpoint"] = nativelink_service.endpoint
            records.append(record)
    finally:
        if fixture.daemon and tool.name == "slug":
            _shutdown_slug_daemon(output_base)
        if nativelink_service is not None:
            stop_nativelink(nativelink_service)

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