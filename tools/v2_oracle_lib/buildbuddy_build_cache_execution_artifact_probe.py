"""Fail-closed, metadata-only execution-artifact probe."""
from __future__ import annotations

import os
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Callable

from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_artifact_probe as lifecycle
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as cleanup

REPO_ROOT = cache.REPO_ROOT
MODE = "buildbuddy-build-cache-prime-execution-artifact-probe"
CLASSES = frozenset(("PROBE_RECORDED", "SANITIZER_REJECTED"))
EXECUTIONS = frozenset(("ANCHORED_PRIVATE_NONEMPTY", "ANCHORED_PRIVATE_EMPTY", "NOT_ANCHORED_PRIVATE"))


def record(classification: str = "SANITIZER_REJECTED", process: str = "NONZERO", execution: str = "NOT_ANCHORED_PRIVATE") -> dict[str, object]:
    if any(type(value) is not str for value in (classification, process, execution)) or classification not in CLASSES or process not in ("ZERO", "NONZERO") or execution not in EXECUTIONS or classification == "SANITIZER_REJECTED":
        classification, process, execution = "SANITIZER_REJECTED", "NONZERO", "NOT_ANCHORED_PRIVATE"
    return {"schema_version": 1, "mode": MODE, "classification": classification, "process": process, "execution": execution}


def normalize(value: object) -> dict[str, object]:
    if type(value) is not dict or set(value) != set(record()) or type(value.get("schema_version")) is not int or value.get("schema_version") != 1 or any(type(value.get(key)) is not str for key in ("mode", "classification", "process", "execution")):
        return record()
    result = record(value["classification"], value["process"], value["execution"])
    return result if result == value else record()


def _private(path: Path) -> None:
    cleanup._private_file(path)
    item = path.lstat()
    if not stat.S_ISREG(item.st_mode) or stat.S_IMODE(item.st_mode) != 0o600 or item.st_nlink != 1:
        raise OSError


def _anchored(root: Path, root_fd: int, root_id: tuple[int, int], phase_id: tuple[int, int]) -> bool:
    try:
        disk, opened, phase = root.lstat(), os.fstat(root_fd), os.stat("prime", dir_fd=root_fd, follow_symlinks=False)
        return stat.S_ISDIR(disk.st_mode) and stat.S_ISDIR(phase.st_mode) and (disk.st_dev, disk.st_ino) == (opened.st_dev, opened.st_ino) == root_id and (phase.st_dev, phase.st_ino) == phase_id
    except OSError:
        return False


def _execution(phase_fd: int, name: str) -> str:
    try:
        item = os.stat(name, dir_fd=phase_fd, follow_symlinks=False)
        if not stat.S_ISREG(item.st_mode) or stat.S_IMODE(item.st_mode) != 0o600 or item.st_nlink != 1:
            return "NOT_ANCHORED_PRIVATE"
        return "ANCHORED_PRIVATE_NONEMPTY" if item.st_size else "ANCHORED_PRIVATE_EMPTY"
    except OSError:
        return "NOT_ANCHORED_PRIVATE"


def _same_output(phase_fd: int, output_fd: int, identity: tuple[int, int]) -> bool:
    try:
        opened = os.fstat(output_fd); current = os.stat("output", dir_fd=phase_fd, follow_symlinks=False)
        return stat.S_ISDIR(opened.st_mode) and stat.S_ISDIR(current.st_mode) and (opened.st_dev, opened.st_ino) == identity == (current.st_dev, current.st_ino)
    except OSError:
        return False


def _remove_reserved(root: Path, parent_fd: int) -> bool:
    try:
        item = os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return True
    except OSError:
        return False
    if stat.S_ISDIR(item.st_mode):
        return cleanup._remove_root(root)
    try:
        os.unlink(root.name, dir_fd=parent_fd)
        os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
        return False
    except FileNotFoundError:
        return True
    except OSError:
        return False


def _shutdown(bazel: str, output: Path, runner: Callable[..., subprocess.CompletedProcess[bytes]]) -> bool:
    try:
        return runner([bazel, "--ignore_all_rc_files", f"--output_base={output}", "shutdown"], cwd=REPO_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False).returncode == 0
    except Exception:
        return False


def run_probe(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, object]:
    previous, root, parent_fd, root_fd, phase_fd, output_fd, root_id, phase_id, output_id, result = os.umask(0o077), None, None, None, None, None, None, None, None, record()
    try:
        if not (cleanup._clean_git() and cleanup._no_slugd()): raise OSError
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700: raise OSError
        try: root.resolve().relative_to(REPO_ROOT.resolve()); raise OSError
        except ValueError: pass
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        root_item = os.fstat(root_fd); root_id = (root_item.st_dev, root_item.st_ino)
        phase = root / "prime"; phase.mkdir(); phase_fd = os.open("prime", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
        phase_item = os.fstat(phase_fd); phase_id = (phase_item.st_dev, phase_item.st_ino)
        output = phase / "output"; output.mkdir(); output_fd = os.open("output", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=phase_fd)
        output_item = os.fstat(output_fd); output_id = (output_item.st_dev, output_item.st_ino)
        execution, bep = phase / "execution.json", phase / "bep.json"
        _private(execution)
        with (phase / "stdout").open("xb") as out, (phase / "stderr").open("xb") as err:
            done = runner(cache.command("prime", bazel, output, bep, execution, secrets.token_hex(32)), cwd=REPO_ROOT, stdout=out, stderr=err, check=False)
        if not _anchored(root, root_fd, root_id, phase_id) or not _same_output(phase_fd, output_fd, output_id): raise OSError
        process = "ZERO" if type(done.returncode) is int and done.returncode == 0 else "NONZERO"
        execution_class = _execution(phase_fd, execution.name)
        if not _anchored(root, root_fd, root_id, phase_id) or not _same_output(phase_fd, output_fd, output_id): raise OSError
        result = record("PROBE_RECORDED", process, execution_class)
    except Exception:
        result = record()
    finally:
        os.umask(previous); okay = root is not None and root_fd is not None and phase_fd is not None and output_fd is not None and root_id is not None and phase_id is not None and output_id is not None
        if okay and _anchored(root, root_fd, root_id, phase_id) and _same_output(phase_fd, output_fd, output_id):
            okay = _shutdown(bazel, root / "prime" / "output", runner) and _anchored(root, root_fd, root_id, phase_id) and _same_output(phase_fd, output_fd, output_id)
        else: okay = False
        if output_fd is not None: os.close(output_fd)
        if phase_fd is not None: os.close(phase_fd)
        if parent_fd is not None and root_fd is not None and root_id is not None: okay &= lifecycle._remove_original(parent_fd, root_fd, root_id)
        else: okay = False
        if root is not None and parent_fd is not None: okay &= _remove_reserved(root, parent_fd)
        if root_fd is not None: os.close(root_fd)
        if parent_fd is not None: os.close(parent_fd)
        if not (cleanup._clean_git() and cleanup._no_slugd()): okay = False
        if not okay: result = record()
    return result
