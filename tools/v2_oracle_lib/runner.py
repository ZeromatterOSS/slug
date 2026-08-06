from __future__ import annotations

import hashlib
import ipaddress
import json
import os
import re
import select
import shutil
import socket
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
from .payload import extract as extract_payload_workspace
from .payload import projection

REPO_ROOT = Path(__file__).resolve().parents[2]
HTTP_REGISTRY_STARTUP_TIMEOUT_SECONDS = 5.0
DAEMON_SHUTDOWN_TIMEOUT_SECONDS = 10.0
WORKSPACE_URI_TOKEN = "{{workspace_uri}}"
RC_ANNOUNCEMENT_RE = re.compile(
    r"Reading 'startup' options from (?P<source>.+): (?P<options>.+)"
)


@dataclass(frozen=True)
class ToolConfig:
    name: str
    executable: Path


@dataclass(frozen=True)
class RunOptions:
    run_root: Path
    timeout_seconds: int = 120


@dataclass(frozen=True)
class BazelServerIdentity:
    pid: int
    start_time: str
    endpoint: tuple[str, int]

    @property
    def process_key(self) -> tuple[int, str]:
        return (self.pid, self.start_time)


class ServerEpochs:
    def __init__(self) -> None:
        self._epochs: dict[tuple[int, str], tuple[int, BazelServerIdentity]] = {}

    def observe(self, identity: BazelServerIdentity) -> int:
        key = identity.process_key
        existing = self._epochs.get(key)
        if existing is not None:
            epoch, prior = existing
            if prior.endpoint != identity.endpoint:
                raise RuntimeError(
                    "Bazel server endpoint changed for live PID/starttime identity"
                )
            return epoch
        epoch = len(self._epochs) + 1
        self._epochs[key] = (epoch, identity)
        return epoch

    @property
    def identities(self) -> tuple[BazelServerIdentity, ...]:
        return tuple(value[1] for value in self._epochs.values())


def default_run_root() -> Path:
    configured = os.environ.get("SLUG_V2_ORACLE_ROOT")
    if configured:
        return Path(configured)
    return REPO_ROOT / "target" / "v2o"


def _copy_workspace(fixture: Fixture, run_dir: Path) -> Path:
    workspace = run_dir / "workspace"
    if workspace.exists():
        raise FileExistsError(f"refusing to reuse run workspace: {workspace}")
    if fixture.payload_workspace is None:
        shutil.copytree(fixture.workspace, workspace, symlinks=True)
    else:
        payload = REPO_ROOT / "tests" / "v2_fixture_payload" / "fixtures.payload"
        payload_bytes = payload.read_bytes()
        actual_hash = hashlib.sha256(
            projection(payload_bytes, fixture.payload_workspace)
        ).hexdigest()
        if actual_hash != fixture.initial_tree_hash:
            raise ValueError(
                f"fixture payload projection hash mismatch for {fixture.name}: {actual_hash}"
            )
        extract_payload_workspace(payload_bytes, fixture.payload_workspace, workspace)
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
        if mutation.op in {"raw_create", "raw_delete"}:
            applied.append(_apply_raw_file_mutation(workspace, mutation))
            continue
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
        if mutation.op == "fifo":
            if os.name != "posix":
                raise RuntimeError("fifo mutation requires a POSIX host")
            if os.path.lexists(path):
                raise FileExistsError(
                    f"mutation fifo destination exists: {mutation.path}"
                )
            _require_existing_real_parent(path, mutation.path)
            created = False
            try:
                os.mkfifo(path, 0o600)
                created = True
                os.chmod(path, 0o600)
                mode = path.lstat().st_mode
                if not stat.S_ISFIFO(mode) or stat.S_IMODE(mode) != 0o600:
                    raise RuntimeError(
                        f"mutation fifo did not retain FIFO type and mode 0600: {mutation.path}"
                    )
            except BaseException:
                if created:
                    path.unlink(missing_ok=True)
                raise
            applied.append({"op": "fifo", "path": mutation.path})
            continue
        if mutation.op == "delete":
            _require_regular_or_symlink_source(path, mutation.path)
            path.unlink()
            applied.append({"op": "delete", "path": mutation.path})
            continue
        if mutation.op == "rename":
            assert mutation.destination is not None
            destination = _workspace_mutation_entry_path(workspace, mutation.destination)
            _require_rename_source(path, mutation.path)
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


def _apply_raw_file_mutation(
    workspace: Path, mutation: Mutation
) -> dict[str, str | None]:
    if sys.platform != "linux":
        raise RuntimeError("raw filename mutation requires a Linux host")
    assert mutation.op in {"raw_create", "raw_delete"}
    assert mutation.name_bytes_hex is not None
    name = bytes.fromhex(mutation.name_bytes_hex)
    parent_fd = _open_raw_mutation_parent(workspace, mutation.path)
    try:
        if mutation.op == "raw_create":
            assert mutation.content is not None
            try:
                file_fd = os.open(
                    name,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                    0o666,
                    dir_fd=parent_fd,
                )
            except FileExistsError as error:
                raise FileExistsError(
                    "raw mutation create destination exists: "
                    f"{mutation.path}/<{mutation.name_bytes_hex}>"
                ) from error
            try:
                with os.fdopen(file_fd, "wb") as output:
                    output.write(mutation.content.encode("utf-8"))
            except BaseException:
                try:
                    os.unlink(name, dir_fd=parent_fd)
                except FileNotFoundError:
                    pass
                raise
        else:
            try:
                mode = os.stat(name, dir_fd=parent_fd, follow_symlinks=False).st_mode
            except FileNotFoundError as error:
                raise FileNotFoundError(
                    "raw mutation source does not exist: "
                    f"{mutation.path}/<{mutation.name_bytes_hex}>"
                ) from error
            if not stat.S_ISREG(mode):
                raise ValueError(
                    "raw mutation source must be a regular file: "
                    f"{mutation.path}/<{mutation.name_bytes_hex}>"
                )
            os.unlink(name, dir_fd=parent_fd)
    finally:
        os.close(parent_fd)
    return {
        "op": mutation.op,
        "path": mutation.path,
        "name_bytes_hex": mutation.name_bytes_hex,
    }


def _open_raw_mutation_parent(workspace: Path, path: str) -> int:
    workspace_root = workspace.resolve()
    candidate = workspace / path
    resolved = candidate.resolve(strict=False)
    if not resolved.is_relative_to(workspace_root):
        raise ValueError(f"raw mutation parent escapes workspace: {path}")

    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    current_fd = os.open(os.fsencode(workspace_root), directory_flags)
    try:
        for component in Path(path).parts:
            if component == ".":
                continue
            try:
                next_fd = os.open(
                    component.encode("ascii"),
                    directory_flags,
                    dir_fd=current_fd,
                )
            except OSError as error:
                raise FileNotFoundError(
                    "raw mutation parent must be an existing real directory: "
                    f"{path}"
                ) from error
            os.close(current_fd)
            current_fd = next_fd
    except BaseException:
        os.close(current_fd)
        raise
    return current_fd


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


def _require_rename_source(path: Path, display_path: str) -> None:
    if not os.path.lexists(path):
        raise FileNotFoundError(f"mutation source does not exist: {display_path}")
    mode = path.lstat().st_mode
    if not (
        stat.S_ISREG(mode)
        or stat.S_ISLNK(mode)
        or stat.S_ISDIR(mode)
        or stat.S_ISFIFO(mode)
    ):
        raise ValueError(
            "mutation rename source must be a regular file, symlink, directory, "
            f"or FIFO: {display_path}"
        )


def _require_existing_real_parent(path: Path, display_path: str) -> None:
    parent = path.parent
    if not parent.is_dir() or parent.is_symlink():
        raise FileNotFoundError(
            f"mutation destination parent must be an existing real directory: {display_path}"
        )


def _argv(
    tool: ToolConfig,
    fixture: Fixture,
    command: FixtureCommand,
    output_base: Path,
) -> list[str]:
    startup = [*fixture.startup_argv, *command.startup_argv]
    if tool.name == "bazel" or fixture.daemon:
        return [
            str(tool.executable),
            f"--output_base={output_base}",
            *startup,
            *command.argv,
        ]
    return [str(tool.executable), *startup, *command.argv]


def _normalize_diagnostic_text(
    value: str, replacements: dict[str, str]
) -> str:
    normalized = value.replace("\\", "/")
    for raw, token in sorted(
        replacements.items(), key=lambda item: len(item[0]), reverse=True
    ):
        if raw:
            normalized = normalized.replace(raw.replace("\\", "/"), token)
            normalized = normalized.replace(raw, token)
    return normalized


def _extract_startup_diagnostics(
    bep_path: Path,
    stderr: str,
    replacements: dict[str, str],
) -> tuple[list[str], list[str]]:
    if not bep_path.is_file():
        raise RuntimeError(f"startup BEP file was not created: {bep_path}")
    events: list[dict[str, Any]] = []
    for line_number, line in enumerate(
        bep_path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise RuntimeError(
                f"startup BEP line {line_number} is not valid JSON"
            ) from error
        if not isinstance(event, dict):
            raise RuntimeError(f"startup BEP line {line_number} must be an object")
        events.append(event)

    originals: list[dict[str, Any]] = []
    for event in events:
        event_id = event.get("id")
        if not isinstance(event_id, dict):
            continue
        structured_id = event_id.get("structuredCommandLine")
        if not isinstance(structured_id, dict):
            continue
        if structured_id.get("commandLineLabel") != "original":
            continue
        structured = event.get("structuredCommandLine")
        if not isinstance(structured, dict):
            raise RuntimeError("original structuredCommandLine payload must be an object")
        if structured.get("commandLineLabel") != "original":
            raise RuntimeError("original structuredCommandLine payload label must be original")
        originals.append(structured)
    if len(originals) != 1:
        raise RuntimeError(
            f"startup BEP requires exactly one original structuredCommandLine event, got {len(originals)}"
        )

    sections = originals[0].get("sections")
    if not isinstance(sections, list):
        raise RuntimeError("original structuredCommandLine sections must be a list")
    if not all(isinstance(section, dict) for section in sections):
        raise RuntimeError("original structuredCommandLine section must be an object")
    startup_sections = [
        section
        for section in sections
        if section.get("sectionLabel") == "startup options"
    ]
    if len(startup_sections) != 1:
        raise RuntimeError(
            f"startup BEP requires exactly one startup options section, got {len(startup_sections)}"
        )
    option_list = startup_sections[0].get("optionList")
    if not isinstance(option_list, dict):
        raise RuntimeError("startup options optionList must be an object")
    options = option_list.get("option")
    if not isinstance(options, list):
        raise RuntimeError("startup options optionList.option must be a list")
    combined_forms: list[str] = []
    for index, option in enumerate(options):
        if not isinstance(option, dict):
            raise RuntimeError(f"startup option {index} must be an object")
        if "combinedForm" not in option and option:
            raise RuntimeError(f"startup option {index} missing combinedForm must be empty")
        combined_form = option.get("combinedForm", "")
        if not isinstance(combined_form, str):
            raise RuntimeError(f"startup option {index} combinedForm must be a string")
        combined_forms.append(
            _normalize_diagnostic_text(combined_form, replacements)
        )

    announcements: list[str] = []
    for line in stderr.replace("\r\n", "\n").replace("\r", "\n").splitlines():
        if not line.startswith("INFO: "):
            continue
        message = line.removeprefix("INFO: ")
        if RC_ANNOUNCEMENT_RE.fullmatch(message) is None:
            continue
        announcements.append(_normalize_diagnostic_text(message, replacements))
    return combined_forms, announcements


def _parse_loopback_endpoint(raw: str) -> tuple[str, int]:
    value = raw.strip()
    if value.startswith("["):
        closing = value.find("]")
        if closing < 0 or closing + 1 >= len(value) or value[closing + 1] != ":":
            raise RuntimeError(f"invalid Bazel server endpoint: {raw!r}")
        host = value[1:closing]
        port_text = value[closing + 2 :]
    else:
        if value.count(":") != 1:
            raise RuntimeError(f"invalid Bazel server endpoint: {raw!r}")
        host, port_text = value.rsplit(":", 1)
    try:
        address = ipaddress.ip_address(host)
        port = int(port_text)
    except ValueError as error:
        raise RuntimeError(f"invalid Bazel server endpoint: {raw!r}") from error
    if not address.is_loopback or not 1 <= port <= 65535:
        raise RuntimeError(f"invalid Bazel server endpoint: {raw!r}")
    return (address.compressed, port)


def _process_start_time(pid: int) -> str | None:
    try:
        fields = Path(f"/proc/{pid}/stat").read_text(encoding="utf-8").split()
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return None
    if len(fields) < 22:
        return None
    return fields[21]


def _process_identity_alive(identity: BazelServerIdentity) -> bool:
    return _process_start_time(identity.pid) == identity.start_time


def _endpoint_reachable(endpoint: tuple[str, int]) -> bool:
    try:
        with socket.create_connection(endpoint, timeout=0.2):
            return True
    except OSError:
        return False


def _read_bazel_server_identity(output_base: Path) -> BazelServerIdentity:
    server = output_base / "server"
    pid_path = server / "server.pid.txt"
    start_time_path = server / "server.starttime"
    endpoint_path = server / "command_port"
    for path in (pid_path, start_time_path, endpoint_path):
        if not path.is_file():
            raise RuntimeError(f"missing Bazel server metadata: {path.name}")
    try:
        pid = int(pid_path.read_text(encoding="utf-8").strip())
    except ValueError as error:
        raise RuntimeError("invalid Bazel server.pid.txt") from error
    if pid <= 0:
        raise RuntimeError("invalid Bazel server.pid.txt")
    start_time = start_time_path.read_text(encoding="utf-8").strip()
    if not start_time:
        raise RuntimeError("invalid Bazel server.starttime")
    endpoint = _parse_loopback_endpoint(
        endpoint_path.read_text(encoding="utf-8")
    )
    identity = BazelServerIdentity(pid, start_time, endpoint)
    if not _process_identity_alive(identity):
        raise RuntimeError("stale Bazel server PID/starttime identity")
    if not _endpoint_reachable(endpoint):
        raise RuntimeError("live Bazel server endpoint is unreachable")
    return identity


def _wait_for_bazel_shutdown(
    identities: tuple[BazelServerIdentity, ...], timeout_seconds: float
) -> None:
    deadline = time.monotonic() + timeout_seconds
    while True:
        live = [identity for identity in identities if _process_identity_alive(identity)]
        reachable = [
            identity.endpoint
            for identity in identities
            if _endpoint_reachable(identity.endpoint)
        ]
        if not live and not reachable:
            return
        if time.monotonic() >= deadline:
            raise RuntimeError(
                "Bazel server did not terminate cleanly "
                f"(live identities={len(live)}, reachable endpoints={len(reachable)})"
            )
        time.sleep(0.05)


def _shutdown_bazel_daemon(
    tool: ToolConfig,
    output_base: Path,
    workspace: Path,
    fixture_startup_argv: tuple[str, ...],
    fixture_env: tuple[tuple[str, str], ...],
    identities: tuple[BazelServerIdentity, ...],
    timeout_seconds: float,
) -> None:
    env = os.environ.copy()
    env.update(dict(fixture_env))
    argv = [
        str(tool.executable),
        f"--output_base={output_base}",
        *fixture_startup_argv,
        "shutdown",
    ]
    completed = subprocess.run(
        argv,
        cwd=workspace,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_seconds,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"Bazel shutdown exited with exit code {completed.returncode}: "
            f"{completed.stderr.strip()}"
        )
    _wait_for_bazel_shutdown(identities, timeout_seconds)


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
    if fixture.required_host_os == "posix" and os.name != "posix":
        raise RuntimeError(
            f"fixture {fixture.name} requires a POSIX host"
        )
    if fixture.required_host_os == "linux" and sys.platform != "linux":
        raise RuntimeError(
            f"fixture {fixture.name} requires a Linux host"
        )
    if fixture.observe_server_epochs and tool.name != "bazel":
        raise RuntimeError(
            "server epoch observation is Bazel-only because authenticated Slug Status "
            f"is unavailable; {tool.name} is unsupported"
        )

    run_id = time.strftime("%Y%m%d-%H%M%S") + f"-{os.getpid()}-{tool.name}"
    run_dir = options.run_root / "runs" / fixture.name / run_id
    run_dir.mkdir(parents=True, exist_ok=False)
    workspace = _copy_workspace(fixture, run_dir)
    output_base = options.run_root / "ob" / fixture.name / tool.name
    output_base.mkdir(parents=True, exist_ok=True)
    diagnostic_replacements = path_replacements(
        workspace=workspace, run_dir=run_dir, output_base=output_base
    )
    replacements = dict(diagnostic_replacements)

    nativelink_service: NativeLinkService | None = None
    registry_service: subprocess.Popen[bytes] | None = None
    registry_log: Path | None = None
    registry_endpoint: str | None = None
    epochs = ServerEpochs()
    primary_error: BaseException | None = None
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
        for command_index, command in enumerate(fixture.commands, start=1):
            mutations = _apply_mutations(workspace, command)
            argv = _argv(tool, fixture, command, output_base)
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
            startup_bep: Path | None = None
            if command.capture_startup_diagnostics:
                startup_bep_dir = run_dir / "startup-bep"
                startup_bep_dir.mkdir(exist_ok=True)
                startup_bep = (startup_bep_dir / f"{command_index:02d}.json").resolve()
                argv.append(f"--build_event_json_file={startup_bep}")
            env = os.environ.copy()
            fixture_env_overrides = dict(fixture.env)
            env_overrides = dict(command.env)
            env.update(fixture_env_overrides)
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
            server_epoch: int | None = None
            if fixture.observe_server_epochs:
                identity = _read_bazel_server_identity(output_base)
                server_epoch = epochs.observe(identity)
            startup_combined_forms: list[str] | None = None
            startup_announcements: list[str] | None = None
            if startup_bep is not None:
                startup_combined_forms, startup_announcements = (
                    _extract_startup_diagnostics(
                        startup_bep, completed.stderr, diagnostic_replacements
                    )
                )
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
                "fixture_startup_argv": fixture.startup_argv,
                "startup_argv": command.startup_argv,
                "env_allowlist": {key: env.get(key) for key in command.env_allowlist},
                "fixture_env_overrides": fixture_env_overrides,
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
            if command.capture_server_epoch:
                assert server_epoch is not None
                record["server_epoch"] = server_epoch
            if command.capture_startup_diagnostics:
                assert startup_combined_forms is not None
                assert startup_announcements is not None
                record["startup_combined_forms"] = startup_combined_forms
                record["startup_announcements"] = startup_announcements
            if nativelink_service is not None:
                record["reapi_evidence"] = _extract_reapi_evidence(completed.stderr)
                record["reapi_endpoint"] = nativelink_service.endpoint
            if registry_request_counts is not None:
                record["http_registry_request_counts"] = registry_request_counts
            records.append(record)
    except BaseException as error:
        primary_error = error
        raise
    finally:
        try:
            try:
                if fixture.observe_server_epochs:
                    _shutdown_bazel_daemon(
                        tool,
                        output_base,
                        workspace,
                        fixture.startup_argv,
                        fixture.env,
                        epochs.identities,
                        DAEMON_SHUTDOWN_TIMEOUT_SECONDS,
                    )
                elif fixture.daemon and tool.name == "slug":
                    _shutdown_slug_daemon(output_base)
                if nativelink_service is not None:
                    stop_nativelink(nativelink_service)
                if registry_service is not None:
                    _stop_process(registry_service)
            finally:
                startup_bep_dir = run_dir / "startup-bep"
                if startup_bep_dir.exists():
                    shutil.rmtree(startup_bep_dir)
        except BaseException as cleanup_error:
            if primary_error is not None:
                raise primary_error from cleanup_error
            raise

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
